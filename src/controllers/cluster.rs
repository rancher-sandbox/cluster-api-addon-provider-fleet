use crate::api::bundle_namespace_mapping::BundleNamespaceMapping;
use crate::api::capi_cluster::Cluster;

use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::api::fleet_cluster::{self};

#[cfg(feature = "agent-initiated")]
use crate::api::fleet_cluster_registration_token::ClusterRegistrationToken;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::Error;
use futures::channel::mpsc::Sender;
use futures::StreamExt as _;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{ApiResource, Object, PatchParams};

use kube::core::SelectorExt as _;
use kube::runtime::watcher::{self, Config};
use kube::{api::ResourceExt, runtime::controller::Action, Resource};
use kube::{Api, Client};
#[cfg(feature = "agent-initiated")]
use rand::distr::{Alphanumeric, SampleString as _};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{info, warn};

use std::sync::Arc;
use std::time::Duration;

use super::controller::{
    fetch_config, get_or_create, patch, Context, FleetBundle, FleetController,
};
use super::{BundleResult, ClusterSyncError, ClusterSyncResult, LabelCheckResult};

pub static CONTROLPLANE_READY_CONDITION: &str = "ControlPlaneReady";

pub struct FleetClusterBundle {
    template_sources: TemplateSources,
    fleet: fleet_cluster::Cluster,
    fleet_group: Option<ClusterGroup>,
    mapping: Option<BundleNamespaceMapping>,
    #[cfg(feature = "agent-initiated")]
    cluster_registration_token: Option<ClusterRegistrationToken>,
    config: FleetAddonConfig,
}

pub struct TemplateSources(Cluster);

#[derive(Serialize)]
struct TemplateValues {
    #[serde(rename = "Cluster")]
    cluster: Cluster,
    #[serde(rename = "ControlPlane")]
    control_plane: Object<Value, Value>,
    #[serde(rename = "InfrastructureCluster")]
    infrastructure_cluster: Object<Value, Value>,
}

impl TemplateSources {
    fn new(cluster: &Cluster) -> Self {
        TemplateSources(cluster.clone())
    }

    async fn resolve(&self, client: Client) -> Option<Value> {
        // We need to remove all dynamic or unnessesary values from these resources
        let mut cluster = self.0.clone();

        cluster.status = None;
        cluster.meta_mut().managed_fields = None;

        let mut control_plane: Object<Value, Value> = client
            .fetch(&self.0.spec.control_plane_ref.clone()?)
            .await
            .ok()?;

        control_plane.status = None;
        control_plane.meta_mut().managed_fields = None;

        let mut infrastructure_cluster: Object<Value, Value> = client
            .fetch(&self.0.spec.infrastructure_ref.clone()?)
            .await
            .ok()?;

        infrastructure_cluster.status = None;
        infrastructure_cluster.meta_mut().managed_fields = None;

        let values = TemplateValues {
            cluster,
            control_plane,
            infrastructure_cluster,
        };

        serde_json::to_value(values).ok()
    }
}

impl FleetBundle for FleetClusterBundle {
    #[allow(refining_impl_trait)]
    async fn sync(&self, ctx: Arc<Context>) -> ClusterSyncResult<Action> {
        let mut cluster = self.fleet.clone();

        if let Some(template) = self.template_sources.resolve(ctx.client.clone()).await {
            let template = serde_json::from_value(template)?;
            cluster.spec.template_values = Some(template);
        }

        if let Some(mapping) = self.mapping.as_ref() {
            if self.config.cluster_patch_enabled() {
                let cluster_name = self.fleet.name_any();
                patch(
                    ctx.clone(),
                    mapping,
                    &PatchParams::apply(&format!("cluster-{cluster_name}-addon-provider-fleet")),
                )
                .await
                .map_err(ClusterSyncError::BundleNamespaceMappingError)?;

                let class_namespace = mapping.namespace().unwrap_or_default();
                let cluster_namespace = mapping.name_any();
                info!("Updated BundleNamespaceMapping for cluster {cluster_name} between class namespace: {class_namespace} and cluster namespace: {cluster_namespace}")
            };
        }

        match self.config.cluster_patch_enabled() {
            true => {
                patch(
                    ctx.clone(),
                    &cluster,
                    &PatchParams::apply("addon-provider-fleet"),
                )
                .await?
            }
            false => get_or_create(ctx.clone(), &cluster).await?,
        };

        #[cfg(feature = "agent-initiated")]
        if let Some(cluster_registration_token) = self.cluster_registration_token.as_ref() {
            get_or_create(ctx.clone(), cluster_registration_token).await?;
        }

        if let Some(group) = self.fleet_group.as_ref() {
            let cluster_name = self.fleet.name_any();
            if self.config.cluster_patch_enabled() {
                patch(
                    ctx.clone(),
                    group,
                    &PatchParams::apply(&format!("cluster-{cluster_name}-addon-provider-fleet")),
                )
                .await
                .map_err(ClusterSyncError::GroupPatchError)?;
            };
        }

        Ok(Action::await_change())
    }
}

impl FleetController for Cluster {
    type Bundle = FleetClusterBundle;

    async fn to_bundle(&self, ctx: Arc<Context>) -> BundleResult<Option<FleetClusterBundle>> {
        let config = fetch_config(ctx.clone().client.clone()).await?;

        if ctx.version < 32 && !self.matching_labels(&config, ctx.client.clone()).await? {
            return Ok(None);
        }

        if !config.cluster_operations_enabled() {
            return Ok(None);
        }

        if self.cluster_ready().is_none() {
            return Ok(None);
        }

        Ok(Some(FleetClusterBundle {
            template_sources: TemplateSources::new(self),
            fleet: self.to_cluster(config.spec.cluster.as_ref()),
            fleet_group: self.to_group(config.spec.cluster.as_ref()),
            mapping: self.to_bundle_ns_mapping(config.spec.cluster.as_ref()),
            #[cfg(feature = "agent-initiated")]
            cluster_registration_token: self
                .to_cluster_registration_token(config.spec.cluster.clone()),
            config,
        }))
    }
}

impl Cluster {
    pub fn cluster_ready(&self) -> Option<&Self> {
        let status = self.status.clone()?;
        let cp_ready = status.control_plane_ready.filter(|&ready| ready);
        let ready_condition = status.conditions?.iter().find_map(|c| {
            (c.type_ == CONTROLPLANE_READY_CONDITION && c.status == "True").then_some(true)
        });

        ready_condition.or(cp_ready).map(|_| self)
    }

    pub async fn add_namespace_dynamic_watch(
        ns: Arc<Namespace>,
        ctx: Arc<Context>,
    ) -> crate::Result<Action> {
        ctx.stream.stream.lock().await.push(
            watcher::watcher(
                Api::namespaced_with(
                    ctx.client.clone(),
                    &ns.name_any(),
                    &ApiResource::erase::<Cluster>(&()),
                ),
                Config::default().streaming_lists(),
            )
            .boxed(),
        );

        let name = ns.name_any();
        info!("Reconciled dynamic watches: added namespace watch on {name}");

        Ok(Action::await_change())
    }

    pub async fn matching_labels(
        &self,
        config: &FleetAddonConfig,
        client: Client,
    ) -> LabelCheckResult<bool> {
        let matches = config.cluster_selector()?.matches(self.labels()) || {
            let ns = self.namespace().unwrap_or("default".into());
            let namespace: Namespace = Api::all(client).get(ns.as_str()).await?;
            config.namespace_selector()?.matches(namespace.labels())
        };

        Ok(matches)
    }

    pub async fn reconcile_ns(
        _: Arc<impl Resource>,
        invoke_reconcile: Arc<Mutex<Sender<()>>>,
    ) -> crate::Result<Action> {
        let mut sender = invoke_reconcile.lock().await;
        sender.try_send(())?;
        Ok(Action::await_change())
    }

    pub fn ns_trigger_error_policy(
        _: Arc<impl kube::Resource>,
        error: &Error,
        _: Arc<Mutex<Sender<()>>>,
    ) -> Action {
        warn!("triggrer invocation failed: {:?}", error);
        Action::requeue(Duration::from_secs(5))
    }
}
