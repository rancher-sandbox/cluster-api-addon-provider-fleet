use crate::api::capi_cluster::Cluster;
use crate::api::capi_clusterclass::ClusterClass;
use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::api::fleet_cluster;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::controllers::controller::{Context, FleetController};
use crate::metrics::Diagnostics;
use crate::{Error, Metrics};

use futures::channel::mpsc;
use futures::StreamExt;

use k8s_openapi::api::core::v1::Namespace;
use kube::{
    api::Api,
    client::Client,
    runtime::{
        controller::{Action, Controller},
        watcher::Config,
    },
};
use tokio::sync::Mutex;

use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::{self, warn};

/// State shared between the controller and the web server
#[derive(Clone, Default)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics registry
    registry: prometheus::Registry,
    metrics: Metrics,
}

/// State wrapper around the controller outputs for the web server
impl State {
    pub fn new() -> Self {
        let registry = Default::default();
        Self {
            metrics: Metrics::default().register(&registry).unwrap(),
            registry,
            ..Default::default()
        }
    }

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
            metrics: self.metrics.clone(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

/// Initialize the controller and shared state (given the crd is installed)
pub async fn run_cluster_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let clusters = Api::<Cluster>::all(client.clone());
    let fleet = Api::<fleet_cluster::Cluster>::all(client.clone());

    let config_api: Api<FleetAddonConfig> = Api::all(client.clone());
    let config = config_api
        .get_opt("fleet-addon-config")
        .await
        .expect("failed to get FleetAddonConfig resource")
        .unwrap_or_default();

    let (invoke_reconcile, namespace_trigger) = mpsc::channel(0);
    let clusters = Controller::new(
        clusters,
        Config::default()
            .labels_from(&config.cluster_watch())
            .any_semantic(),
    )
    .owns(fleet, Config::default().any_semantic())
    .reconcile_all_on(namespace_trigger)
    .shutdown_on_signal()
    .run(
        Cluster::reconcile,
        error_policy,
        state.to_context(client.clone()),
    )
    .for_each(|_| futures::future::ready(()));

    if config.namespace_selector().selects_all() {
        return clusters.await;
    }

    let ns_controller = Controller::new(
        Api::<Namespace>::all(client.clone()),
        Config::default()
            .labels_from(&config.namespace_selector())
            .any_semantic(),
    )
    .shutdown_on_signal()
    .run(
        Cluster::reconcile_ns,
        Cluster::ns_trigger_error_policy,
        Arc::new(Mutex::new(invoke_reconcile)),
    )
    .for_each(|_| futures::future::ready(()));

    tokio::join!(clusters, ns_controller);
}

/// Initialize the controller and shared state (given the crd is installed)
pub async fn run_cluster_class_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let cluster_classes = Api::<ClusterClass>::all(client.clone());
    let fleet_groups = Api::<ClusterGroup>::all(client.clone());

    Controller::new(cluster_classes, Config::default().any_semantic())
        .owns(fleet_groups, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            ClusterClass::reconcile,
            error_policy,
            state.to_context(client.clone()),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await
}

fn error_policy(doc: Arc<impl kube::Resource>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(doc, error);
    Action::requeue(Duration::from_secs(5 * 60))
}
