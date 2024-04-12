#![allow(unused_imports, unused_variables, dead_code)]
use crate::api::capi_cluster::{Cluster, ClusterStatus};
use crate::api::fleet_cluster::{Cluster as fleetcluster, ClusterSpec as fleetspec};
use crate::{telemetry, Error, Metrics, Result};
use chrono::{DateTime, Utc};
use futures::StreamExt;
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
    let cfg = Config::default();
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();

    let cluster_name = c.name_any();
    let ns = c.metadata.namespace.as_ref().unwrap();
    let cluster_api: Api<Cluster> = Api::namespaced(ctx.client.clone(), &ns);
    debug!("Reconciling Cluster \"{}\" in {}", cluster_name, ns);
    finalizer(&cluster_api, FLEET_FINALIZER, c, |event| async {
        match event {
            Finalizer::Apply(c) => c.reconcile(ctx.clone()).await,
            Finalizer::Cleanup(c) => c.cleanup(ctx.clone()).await,
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

impl Cluster {
    async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action> {
        let client = ctx.client.clone();
        let _recorder = ctx.diagnostics.read().await.recorder(client.clone(), self);
        //let ns = self.namespace().unwrap();
        let ns = self.metadata.namespace.as_ref().unwrap();
        let name = self.name_any();
        let clusters_api: Api<Cluster> = Api::namespaced(client.clone(), &ns);

        let status = self.status().unwrap();
        if !cluster_ready(status) {
            return Ok(Action::requeue(Duration::from_secs(30)));
        }
        debug!("cluster has control plane ready \"{}\" in {}", name, ns);

        match self.reconcile_fleet_cluster(ctx.clone()).await {
            Err(e) => {
                error!("Failed reconciling fleet cluster: {}", e);
            }
            Ok(_) => {
                debug!("Reconciled Fleet cluster")
            }
        }

        //Ok(Action::requeue(Duration::from_secs(5 * 60)))#
        // let jitter = rand::thread_rng().gen_range(0..60);
        // Ok(Action::requeue(Duration::from_secs(
        //     cfg.reconcile_ttl + jitter,
        // )))
        Ok(Action::await_change())
    }

    async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action> {
        Ok(Action::await_change())
    }

    pub async fn reconcile_fleet_cluster(&self, ctx: Arc<Context>) -> Result<Action> {
        let cluster_name = self.name_any();
        let ns = self.metadata.namespace.as_ref().unwrap();
        let client = ctx.client.clone();

        let fleet_api: Api<fleetcluster> = Api::namespaced(client.clone(), ns);

        let fleet_cluster = match fleet_api.get(&cluster_name).await {
            Ok(obj) => Ok(Some(obj)),
            Err(kubeerror::Api(ErrorResponse { reason, .. })) if &reason == "NotFound" => Ok(None),
            Err(err) => Err(err),
        };

        match fleet_cluster {
            Err(err) => {
                error!("failed gettinmg fleet cluster: {}", err)
            }
            Ok(fc) => match fc {
                Some(_) => {
                    debug!("fleet cluster already exists, do nothing");
                    return Ok(Action::await_change());
                }
                None => debug!("fleet cluster does not exist, creating"),
            },
        }

        let kubeconfig = format!("{}-kubeconfig", cluster_name);
        let fcluster = fleetcluster::new(
            &cluster_name,
            fleetspec {
                agent_affinity: None,
                agent_env_vars: None,
                agent_namespace: None,
                agent_resources: None,
                agent_tolerations: None,
                client_id: None,
                kube_config_secret: Some(kubeconfig),
                kube_config_secret_namespace: None,
                paused: None,
                private_repo_url: None,
                redeploy_agent_generation: None,
                template_values: None,
            },
        );

        //TODO: copy labels from capi cluster

        let pp = PostParams::default();
        match fleet_api.create(&pp, &fcluster).await {
            Err(err) => {
                error!("Failed creating fleet cluster: {}", err);
            }
            Ok(_) => {
                debug!("created fleet cluster");
            }
        }

        Ok(Action::await_change())
    }
}

fn cluster_ready(status: &ClusterStatus) -> bool {
    if let Some(control_plane_ready) = status.control_plane_ready {
        if control_plane_ready {
            return true;
        }
    }

    if let Some(conditions) = &status.conditions {
        let read_condition = conditions.iter().find(|condition| {
            condition.type_ == CONTROLPLANE_READY_CONDITION && condition.status == "True"
        });
        if read_condition.is_some() {
            return true;
        }
    }

    false
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
