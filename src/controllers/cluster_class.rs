use crate::api::capi_clusterclass::ClusterClass;

use crate::api::fleet_addon_config::{ClusterClassConfig, FleetAddonConfig};
use crate::api::fleet_clustergroup::ClusterGroup;

use fleet_api_rs::fleet_clustergroup::{ClusterGroupSelector, ClusterGroupSpec};
use kube::api::{ObjectMeta, PatchParams, TypeMeta};

use kube::{api::ResourceExt, runtime::controller::Action, Resource};

use std::collections::BTreeMap;
use std::sync::Arc;

use super::controller::{
    fetch_config, get_or_create, patch, Context, FleetBundle, FleetController,
};
use super::{BundleResult, GroupSyncResult};

pub static CLUSTER_CLASS_LABEL: &str = "clusterclass-name.fleet.addons.cluster.x-k8s.io";
pub static CLUSTER_CLASS_NAMESPACE_LABEL: &str =
    "clusterclass-namespace.fleet.addons.cluster.x-k8s.io";

pub struct FleetClusterClassBundle {
    fleet_group: ClusterGroup,
    config: FleetAddonConfig,
}

impl From<&ClusterClass> for ClusterGroup {
    fn from(cluster_class: &ClusterClass) -> Self {
        let labels = {
            let mut labels = BTreeMap::default();
            labels.insert(CLUSTER_CLASS_LABEL.to_string(), cluster_class.name_any());
            labels.insert(
                CLUSTER_CLASS_NAMESPACE_LABEL.to_string(),
                cluster_class.namespace().unwrap_or_default(),
            );
            Some(labels)
        };
        Self {
            types: Some(TypeMeta::resource::<ClusterGroup>()),
            metadata: ObjectMeta {
                name: Some(cluster_class.name_any()),
                namespace: cluster_class.meta().namespace.clone(),
                labels: labels.clone(),
                owner_references: cluster_class
                    .owner_ref(&())
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                ..Default::default()
            },
            spec: ClusterGroupSpec {
                selector: Some(ClusterGroupSelector {
                    match_labels: labels,
                    ..Default::default()
                }),
            },
            ..Default::default()
        }
    }
}

impl FleetBundle for FleetClusterClassBundle {
    #[allow(refining_impl_trait)]
    async fn sync(&self, ctx: Arc<Context>) -> GroupSyncResult<Action> {
        match self.config.cluster_class_patch_enabled() {
            true => {
                patch(
                    ctx,
                    self.fleet_group.clone(),
                    &PatchParams::apply("addon-provider-fleet"),
                )
                .await?
            }
            false => get_or_create(ctx.clone(), self.fleet_group.clone()).await?,
        };

        Ok(Action::await_change())
    }
}

impl FleetController for ClusterClass {
    type Bundle = FleetClusterClassBundle;

    async fn to_bundle(&self, ctx: Arc<Context>) -> BundleResult<Option<FleetClusterClassBundle>> {
        let config = fetch_config(ctx.clone().client.clone()).await?;
        if !config.cluster_class_operations_enabled() {
            return Ok(None);
        }

        let mut fleet_group: ClusterGroup = self.into();
        if let Some(ClusterClassConfig {
            set_owner_references: Some(true),
            ..
        }) = config.spec.cluster_class
        {
        } else {
            fleet_group.metadata.owner_references = None
        }

        Ok(Some(FleetClusterClassBundle {
            fleet_group,
            config,
        }))
    }
}
