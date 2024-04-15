#![allow(unused_imports, dead_code)]
use crate::api::capi_cluster::{Cluster, ClusterStatus};
use crate::api::fleet_cluster;
use crate::{telemetry, Error, Metrics, Result};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use kube::api::ObjectMeta;
use kube::runtime::reflector::Lookup;
use kube::{
    api::{Api, ListParams, Patch, PatchParams, PostParams, ResourceExt},
    client::Client,
    core::{object::HasStatus, ErrorResponse},
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
        watcher::Config,
    },
    CustomResource, Error as kubeerror, Resource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::*;

pub static FLEET_FINALIZER: &str = "fleet.addons.cluster.x-k8s.io";
pub static CONTROLPLANE_READY_CONDITION: &str = "ControlPlaneReady";

// Context for the reconciler
#[derive(Clone)]
pub struct Context {
    /// Kubernetes client
    pub client: Client,
    /// Diagnostoics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prom metrics
    pub metrics: Metrics,
}

#[instrument(skip(ctx, c), fields(trace_id))]
async fn reconcile(c: Arc<Cluster>, ctx: Arc<Context>) -> Result<Action> {
    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let _cfg = Config::default();
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();

    let cluster_name = c.name_any();
    let ns = c.metadata.namespace.as_ref().unwrap();
    let cluster_api: Api<Cluster> = Api::namespaced(ctx.client.clone(), ns);
    debug!("Reconciling Cluster \"{}\" in {}", cluster_name, ns);
    finalizer(&cluster_api, FLEET_FINALIZER, c, |event| async {
        let r = match event {
            Finalizer::Apply(c) => c.to_bundle()?.sync_fleet_cluster(ctx).await,
            Finalizer::Cleanup(c) => c.cleanup(ctx.clone()).await,
        };

        match r {
            Ok(r) => Ok(r),
            Err(Error::EarlyReturn) => Ok(Action::await_change()),
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|e| Error::FinalizerError(Box::new(e)))
}

fn error_policy(doc: Arc<Cluster>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(&doc, error);
    Action::requeue(Duration::from_secs(5 * 60))
}

struct FleetClusterBundle {
    cluster: Cluster,
    fleet: fleet_cluster::Cluster,
}

impl From<&Cluster> for FleetClusterBundle {
    fn from(cluster: &Cluster) -> Self {
        Self {
            cluster: cluster.clone(),
            fleet: cluster.into(),
        }
    }
}

impl From<&Cluster> for fleet_cluster::Cluster {
    fn from(cluster: &Cluster) -> Self {
        Self {
            metadata: ObjectMeta {
                labels: Some(cluster.labels().clone()),
                name: Some(cluster.name_any()),
                namespace: cluster.meta().namespace.clone(),
                ..Default::default()
            },
            spec: fleet_cluster::ClusterSpec {
                kube_config_secret: Some(format!("{}-kubeconfig", cluster.name_any())),
                ..Default::default()
            },
            status: Default::default(),
        }
    }
}

impl FleetClusterBundle {
    pub async fn sync_fleet_cluster(&self, ctx: Arc<Context>) -> Result<Action> {
        let ns = self
            .cluster
            .metadata
            .namespace
            .as_ref()
            .ok_or(Error::ClusterNamespaceMissing)?;
        let fleet_api: Api<fleet_cluster::Cluster> = Api::namespaced(ctx.client.clone(), ns);

        let fleet_cluster = match fleet_api.get(self.fleet.name_any().as_str()).await {
            Ok(_) => Err(Error::EarlyReturn),
            Err(kubeerror::Api(ErrorResponse { reason, .. })) if &reason == "NotFound" => {
                Ok(self.fleet.clone())
            }
            Err(err) => Err(err).map_err(Error::FleetClusterLookupError),
        }?;

        let pp = PostParams::default();
        fleet_api
            .create(&pp, &fleet_cluster)
            .await
            .map_err(Error::FleetClusterCreateError)?;

        Ok(Action::await_change())
    }
}

impl Cluster {
    fn to_bundle(&self) -> Result<FleetClusterBundle> {
        self
            .cluster_ready()
            .map(Into::into)
            .ok_or(Error::EarlyReturn)
    }

    pub fn cluster_ready(&self) -> Option<&Self> {
        let cp_ready = self
            .status
            .iter()
            .filter_map(|status| status.control_plane_ready)
            .find(|&ready| ready)
            .map(|_| self);

        let ready_condition = self
            .status
            .iter()
            .filter_map(|status| status.conditions.clone())
            .flatten()
            .find(|c| c.type_ == CONTROLPLANE_READY_CONDITION && c.status == "True")
            .map(|_| self);

        cp_ready.or(ready_condition)
    }

    async fn cleanup(&self, _ctx: Arc<Context>) -> Result<Action> {
        Ok(Action::await_change())
    }
}

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "doc-controller".into(),
        }
    }
}
impl Diagnostics {
    fn recorder(&self, client: Client, cluster: &Cluster) -> Recorder {
        Recorder::new(client, self.reporter.clone(), cluster.object_ref(&()))
    }
}

/// State shared between the controller and the web server
#[derive(Clone, Default)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics registry
    registry: prometheus::Registry,
}

/// State wrapper around the controller outputs for the web server
impl State {
    /// Metrics getter
    pub fn metrics(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    pub fn to_context(&self, client: Client) -> Arc<Context> {
        Arc::new(Context {
            client,
            metrics: Metrics::default().register(&self.registry).unwrap(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

/// Initialize the controller and shared state (given the crd is installed)
pub async fn run(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let clusters = Api::<Cluster>::all(client.clone());
    if let Err(e) = clusters.list(&ListParams::default().limit(1)).await {
        error!("Clusters are not queryable; {e:?}. Is the CRD installed?");
        //info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }
    Controller::new(clusters, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.to_context(client))
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
