use crate::api::capi_clusterclass::ClusterClass;

use crate::api::fleet_clustergroup::{ClusterGroup, ClusterGroupSelector, ClusterGroupSpec};
use crate::Result;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::ObjectMeta;

use kube::{api::ResourceExt, runtime::controller::Action, Resource};

use std::sync::Arc;

use super::controller::{get_or_create, Context, FleetBundle, FleetController};
use super::SyncError;

pub static CLUSTER_CLASS_LABEL: &str = "clusterclass-name.fleet.addons.cluster.x-k8s.io";

pub struct FleetClusterClassBundle {
    fleet_group: ClusterGroup,
}

impl From<&ClusterClass> for FleetClusterClassBundle {
    fn from(cluster_class: &ClusterClass) -> Self {
        Self {
            fleet_group: cluster_class.into(),
        }
    }
}

impl From<&ClusterClass> for ClusterGroup {
    fn from(cluster_class: &ClusterClass) -> Self {
        Self {
            metadata: ObjectMeta {
                name: Some(cluster_class.name_any()),
                namespace: cluster_class.meta().namespace.clone(),
                labels: Some(cluster_class.labels().clone()),
                owner_references: cluster_class
                    .controller_owner_ref(&())
                    .into_iter()
                    .map(|r| OwnerReference {
                        controller: None,
                        ..r
                    })
                    .map(Into::into)
                    .collect(),
                ..Default::default()
            },
            spec: ClusterGroupSpec {
                selector: Some(ClusterGroupSelector {
                    match_labels: Some(
                        [(CLUSTER_CLASS_LABEL.to_string(), cluster_class.name_any())].into(),
                    ),
                    ..Default::default()
                }),
            },
            status: Default::default(),
        }
    }
}

impl FleetBundle for FleetClusterClassBundle {
    async fn sync(&self, ctx: Arc<Context>) -> Result<Action> {
        get_or_create(ctx, self.fleet_group.clone())
            .await
            .map_err(SyncError::GroupSync)
            .map_err(Into::into)
    }
}

impl FleetController for ClusterClass {
    type Bundle = FleetClusterClassBundle;
}
