use crate::api::capi_cluster::Cluster;
use crate::api::capi_clusterclass::ClusterClass;
use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::api::fleet_cluster;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::controllers::addon_config::FleetConfig;
use crate::controllers::controller::{fetch_config, Context, FleetController};
use crate::metrics::Diagnostics;
use crate::multi_dispatcher::{broadcaster, BroadcastStream, MultiDispatcher};
use crate::{Error, Metrics};

use clap::Parser;
use futures::channel::mpsc;
use futures::stream::SelectAll;
use futures::{Stream, StreamExt};

use k8s_openapi::api::core::v1::Namespace;
use kube::api::{DynamicObject, ListParams};
use kube::core::DeserializeGuard;
use kube::runtime::reflector::ObjectRef;
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
use tokio::time::sleep;

use std::future;

use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::{self, info, warn};

type DynamicStream = SelectAll<
    Pin<Box<dyn Stream<Item = Result<watcher::Event<DynamicObject>, watcher::Error>> + Send>>,
>;

/// State shared between the controller and the web server
#[derive(Clone)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics registry
    registry: prometheus::Registry,
    metrics: Metrics,

    /// Additional flags for controller
    pub flags: Flags,

    // dispatcher
    dispatcher: MultiDispatcher,
    // shared stream of dynamic events
    stream: BroadcastStream<DynamicStream>,
}

#[derive(Parser, Debug, Clone, Default)]
pub struct Flags {
    /// helm install allows to select container for performing fleet chart installation
    #[arg(long)]
    pub helm_install: bool,
}

/// State wrapper around the controller outputs for the web server
impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn new() -> Self {
        let registry = Default::default();
        Self {
            metrics: Metrics::default().register(&registry).unwrap(),
            registry,
            flags: Flags::parse(),
            dispatcher: MultiDispatcher::new(128),
            diagnostics: Default::default(),
            stream: BroadcastStream::new(Default::default()),
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
            dispatcher: self.dispatcher.clone(),
            stream: self.stream.clone(),
        })
    }
}

pub async fn run_fleet_addon_config_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let config_controller = Controller::new(
        Api::<FleetAddonConfig>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .watches(
        Api::<DeserializeGuard<FleetConfig>>::all(client.clone()),
        Config::default().fields("metadata.name=fleet-controller"),
        |config| config.0.ok().map(|_| ObjectRef::new("fleet-addon-config")),
    )
    .shutdown_on_signal()
    .run(
        FleetAddonConfig::reconcile_config_sync,
        error_policy,
        state.to_context(client.clone()),
    )
    .for_each(|_| futures::future::ready(()));

    let dynamic_watches_controller = Controller::new(
        Api::<FleetAddonConfig>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .shutdown_on_signal()
    .run(
        FleetAddonConfig::reconcile_dynamic_watches,
        error_policy,
        state.to_context(client.clone()),
    )
    .for_each(|_| futures::future::ready(()));

    let watcher = broadcaster(state.dispatcher.clone(), state.stream.clone())
        .for_each(|_| futures::future::ready(()));

    // Reconcile initial state of watches
    Arc::new(
        fetch_config(client.clone())
            .await
            .expect("failed to get FleetAddonConfig resource"),
    )
    .update_watches(state.to_context(client.clone()))
    .await
    .expect("Initial dynamic watches setup to succeed");

    tokio::select! {
        _ = watcher => {panic!("This should not happen before controllers exit")},
        _ = futures::future::join(dynamic_watches_controller, config_controller) => {}
    };
}

pub async fn run_fleet_helm_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let api: Api<FleetAddonConfig> = Api::all(client.clone());
    let fleet_addon_config_controller = Controller::new(api, watcher::Config::default())
        .shutdown_on_signal()
        .run(
            FleetAddonConfig::reconcile_helm,
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

    loop {
        let clusters = Api::<fleet_cluster::Cluster>::all(client.clone());
        if let Err(e) = clusters.list(&ListParams::default().limit(1)).await {
            info!("Fleet Clusters are not queryable; {e:?}. Is the CRD installed?");
            sleep(Duration::new(5, 0)).await;
            continue;
        }

        break;
    }

    let config = fetch_config(client.clone())
        .await
        .expect("failed to get FleetAddonConfig resource");

    let fleet = metadata_watcher(
        Api::<fleet_cluster::Cluster>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .modify(|g| g.managed_fields_mut().clear())
    .touched_objects()
    .predicate_filter(predicates::resource_version);

    let (invoke_reconcile, namespace_trigger) = mpsc::channel(0);
    let (sub, reader) = state.dispatcher.subscribe();
    let clusters = Controller::for_shared_stream(sub, reader)
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

    let (sub, reader) = state.dispatcher.subscribe::<Namespace>();
    let ns_controller = Controller::for_shared_stream(sub, reader)
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
    Action::requeue(Duration::from_secs(10))
}
