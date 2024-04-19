use crate::api::capi_cluster::{Cluster, ClusterTopology};

use crate::api::fleet_cluster;

use crate::Result;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::ObjectMeta;

use kube::{api::ResourceExt, runtime::controller::Action, Resource};

use std::sync::Arc;

use super::cluster_class::CLUSTER_CLASS_LABEL;
use super::controller::{get_or_create, Context, FleetBundle, FleetController};
use super::SyncError;

pub static CONTROLPLANE_READY_CONDITION: &str = "ControlPlaneReady";

pub struct FleetClusterBundle {
    fleet: fleet_cluster::Cluster,
}

impl From<&Cluster> for FleetClusterBundle {
    fn from(cluster: &Cluster) -> Self {
        Self {
            fleet: cluster.into(),
        }
    }
}

impl From<&Cluster> for fleet_cluster::Cluster {
    fn from(cluster: &Cluster) -> Self {
        let labels = match &cluster.spec.topology {
            Some(ClusterTopology { class, .. }) if !class.is_empty() => {
                let mut labels = cluster.labels().clone();
                labels.insert(CLUSTER_CLASS_LABEL.to_string(), class.clone());
                labels
            }
            None | Some(ClusterTopology { .. }) => cluster.labels().clone(),
        };

        Self {
            metadata: ObjectMeta {
                labels: Some(labels),
                name: Some(cluster.name_any()),
                namespace: cluster.meta().namespace.clone(),
                owner_references: cluster
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
            spec: fleet_cluster::ClusterSpec {
                kube_config_secret: Some(format!("{}-kubeconfig", cluster.name_any())),
                ..Default::default()
            },
            status: Default::default(),
        }
    }
}

impl FleetBundle for FleetClusterBundle {
    async fn sync(&self, ctx: Arc<Context>) -> Result<Action> {
        get_or_create(ctx, self.fleet.clone())
            .await
            .map_err(SyncError::ClusterSync)
            .map_err(Into::into)
    }
}

impl FleetController for Cluster {
    type Bundle = FleetClusterBundle;

    fn to_bundle(&self) -> Result<&Self> {
        self.cluster_ready().ok_or(SyncError::EarlyReturn.into())
    }
}

impl Cluster {
    pub fn cluster_ready(&self) -> Option<&Self> {
        let status = self.status.clone()?;
        let cp_ready = status.control_plane_ready.filter(|&ready| ready);
        let ready_condition = status
            .conditions?
            .iter()
            .map(|c| c.type_ == CONTROLPLANE_READY_CONDITION && c.status == "True")
            .find(|&ready| ready);

        ready_condition.or(cp_ready).map(|_| self)
    }
}
