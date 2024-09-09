use crate::api::capi_cluster::Cluster;
use crate::api::capi_clusterclass::ClusterClass;
use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::api::fleet_cluster;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::controllers::controller::{fetch_config, Context, FleetController};
use crate::metrics::Diagnostics;
use crate::{Error, Metrics};

use futures::channel::mpsc;
use futures::StreamExt;

use k8s_openapi::api::core::v1::Namespace;
use kube::runtime::{metadata_watcher, predicates, reflector, watcher, WatchStreamExt};
use kube::ResourceExt as _;
use kube::{
    api::Api,
    client::Client,
    runtime::{
        controller::{Action, Controller},
        watcher::Config,
    },
};
use tokio::sync::Mutex;

use std::future;

use std::ops::Deref;
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

pub async fn run_fleet_addon_config_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let api: Api<FleetAddonConfig> = Api::all(client.clone());
    let fleet_addon_config_controller = Controller::new(api, watcher::Config::default())
        .run(
            FleetAddonConfig::reconcile,
            error_policy,
            state.to_context(client.clone()),
        )
        .for_each(|_| futures::future::ready(()));

    tokio::join!(fleet_addon_config_controller);
}

/// Initialize the controller and shared state (given the crd is installed)
pub async fn run_cluster_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let config = fetch_config(client.clone())
        .await
        .expect("failed to get FleetAddonConfig resource");

    let (reader, writer) = reflector::store();
    let clusters = watcher(
        Api::<Cluster>::all(client.clone()),
        Config::default()
            .labels_from(
                &config
                    .cluster_watch()
                    .expect("valid cluster label selector"),
            )
            .any_semantic(),
    )
    .default_backoff()
    .modify(|c| {
        c.managed_fields_mut().clear();
    })
    .reflect(writer)
    .touched_objects()
    .predicate_filter(predicates::resource_version);

    let fleet = metadata_watcher(
        Api::<fleet_cluster::Cluster>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .modify(|g| g.managed_fields_mut().clear())
    .touched_objects()
    .predicate_filter(predicates::resource_version);

    let (invoke_reconcile, namespace_trigger) = mpsc::channel(0);
    let clusters = Controller::for_stream(clusters, reader)
        .owns_stream(fleet)
        .reconcile_all_on(namespace_trigger)
        .shutdown_on_signal()
        .run(
            Cluster::reconcile,
            error_policy,
            state.to_context(client.clone()),
        )
        .for_each(|_| futures::future::ready(()));

    if config
        .namespace_selector()
        .expect("valid namespace selector")
        .selects_all()
    {
        return clusters.await;
    }

    let (reader, writer) = reflector::store();
    let namespaces = metadata_watcher(
        Api::<Namespace>::all(client.clone()),
        Config::default()
            .labels_from(
                &config
                    .namespace_selector()
                    .expect("valid namespace selector"),
            )
            .any_semantic(),
    )
    .default_backoff()
    .modify(|ns| {
        ns.managed_fields_mut().clear();
        ns.annotations_mut().clear();
        ns.labels_mut().clear();
    })
    .reflect(writer)
    .touched_objects()
    .predicate_filter(predicates::resource_version);

    let ns_controller = Controller::for_stream(namespaces, reader)
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

    let (reader, writer) = reflector::store_shared(1024);
    let subscriber = writer
        .subscribe()
        .expect("subscribe for cluster group updates successfully");
    let fleet_groups = watcher(
        Api::<ClusterGroup>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .default_backoff()
    .modify(|cg| {
        cg.managed_fields_mut().clear();
        cg.status = None;
    })
    .reflect_shared(writer)
    .touched_objects()
    .predicate_filter(predicates::resource_version)
    .for_each(|_| futures::future::ready(()));

    let group_controller = Controller::for_shared_stream(subscriber.clone(), reader)
        .shutdown_on_signal()
        .run(
            ClusterGroup::reconcile,
            error_policy,
            state.to_context(client.clone()),
        )
        .for_each(|_| futures::future::ready(()));

    let (reader, writer) = reflector::store();
    let cluster_classes = watcher(
        Api::<ClusterClass>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .default_backoff()
    .modify(|cc| cc.managed_fields_mut().clear())
    .reflect(writer)
    .touched_objects()
    .predicate_filter(predicates::resource_version);

    let filtered = subscriber
        .map(|s| Ok(s.deref().clone()))
        .predicate_filter(crate::predicates::generation_with_deletion)
        .filter_map(|s| future::ready(s.ok().map(Arc::new)));
    let cluster_class_controller = Controller::for_stream(cluster_classes, reader)
        .owns_shared_stream(filtered)
        .shutdown_on_signal()
        .run(
            ClusterClass::reconcile,
            error_policy,
            state.to_context(client.clone()),
        )
        .for_each(|_| futures::future::ready(()));

    tokio::select! {
        _ = fleet_groups => {},
        _ = futures::future::join(group_controller, cluster_class_controller) => {},
    };
}

fn error_policy(doc: Arc<impl kube::Resource>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(doc, error);
    Action::requeue(Duration::from_secs(5 * 60))
}
