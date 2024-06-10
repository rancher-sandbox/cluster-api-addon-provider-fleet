use crate::api::capi_cluster::{Cluster, ClusterTopology};

use crate::api::fleet_addon_config::{ClusterConfig, FleetAddonConfig};
use crate::api::fleet_cluster;

#[cfg(feature = "agent-initiated")]
use crate::api::fleet_cluster_registration_token::{
    ClusterRegistrationToken, ClusterRegistrationTokenSpec,
};
use crate::Result;
use kube::api::ObjectMeta;

use kube::{api::ResourceExt, runtime::controller::Action, Resource};
#[cfg(feature = "agent-initiated")]
use rand::distributions::{Alphanumeric, DistString as _};

use std::sync::Arc;

use super::cluster_class::CLUSTER_CLASS_LABEL;
use super::controller::{get_or_create, patch, Context, FleetBundle, FleetController};
use super::{ClusterSyncError, SyncError};

pub static CONTROLPLANE_READY_CONDITION: &str = "ControlPlaneReady";

pub struct FleetClusterBundle {
    fleet: fleet_cluster::Cluster,
    #[cfg(feature = "agent-initiated")]
    cluster_registration_token: Option<ClusterRegistrationToken>,
    config: FleetAddonConfig,
}

impl From<&Cluster> for ObjectMeta {
    fn from(cluster: &Cluster) -> Self {
        Self {
            name: Some(cluster.name_any()),
            namespace: cluster.meta().namespace.clone(),
            ..Default::default()
        }
    }
}

impl Cluster {
    fn to_cluster(self: &Cluster, config: Option<ClusterConfig>) -> fleet_cluster::Cluster {
        let config = config.unwrap_or_default();
        let labels = match &self.spec.topology {
            Some(ClusterTopology { class, .. }) if !class.is_empty() => {
                let mut labels = self.labels().clone();
                labels.insert(CLUSTER_CLASS_LABEL.to_string(), class.clone());
                labels
            }
            None | Some(ClusterTopology { .. }) => self.labels().clone(),
        };

        fleet_cluster::Cluster {
            metadata: ObjectMeta {
                labels: Some(labels),
                owner_references: config
                    .set_owner_references
                    .is_some_and(|set| set)
                    .then_some(self.owner_ref(&()).into_iter().collect()),
                name: config.naming.apply(self.name_any().into()),
                ..self.into()
            },
            #[cfg(feature = "agent-initiated")]
            spec: match config.agent_initiated {
                Some(true) => fleet_cluster::ClusterSpec {
                    client_id: Some(Alphanumeric.sample_string(&mut rand::thread_rng(), 64)),
                    ..Default::default()
                },
                None | Some(false) => fleet_cluster::ClusterSpec {
                    kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                    ..Default::default()
                },
            },
            #[cfg(not(feature = "agent-initiated"))]
            spec: fleet_cluster::ClusterSpec {
                kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                ..Default::default()
            },
            status: Default::default(),
        }
    }

    #[cfg(feature = "agent-initiated")]
    fn to_cluster_registration_token(
        self: &Cluster,
        config: Option<ClusterConfig>,
    ) -> Option<ClusterRegistrationToken> {
        config?.agent_initiated?.then_some(true)?;

        ClusterRegistrationToken {
            metadata: self.into(),
            spec: ClusterRegistrationTokenSpec {
                ttl: Some("1h".into()),
            },
            ..Default::default()
        }
        .into()
    }
}

impl FleetBundle for FleetClusterBundle {
    async fn sync(&self, ctx: Arc<Context>) -> Result<Action> {
        get_or_create(ctx.clone(), self.fleet.clone())
            .await
            .map_err(Into::<ClusterSyncError>::into)
            .map_err(Into::<SyncError>::into)?;

        if let Some(true) = self.config.spec.patch_resource {
            patch(ctx.clone(), self.fleet.clone())
                .await
                .map_err(Into::<ClusterSyncError>::into)
                .map_err(Into::<SyncError>::into)?;
        }

        #[cfg(feature = "agent-initiated")]
        get_or_create(
            ctx,
            self.cluster_registration_token
                .clone()
                .ok_or(SyncError::EarlyReturn)?,
        )
        .await
        .map_err(Into::<crate::SyncError>::into)?;

        Ok(Action::await_change())
    }
}

impl FleetController for Cluster {
    type Bundle = FleetClusterBundle;

    fn to_bundle(&self, config: &FleetAddonConfig) -> Result<FleetClusterBundle> {
        config
            .spec
            .cluster
            .iter()
            .filter_map(|c| c.enabled)
            .find(|&enabled| enabled)
            .ok_or(SyncError::EarlyReturn)?;

        self.cluster_ready().ok_or(SyncError::EarlyReturn)?;

        Ok(FleetClusterBundle {
            fleet: self.to_cluster(config.spec.cluster.clone()),
            #[cfg(feature = "agent-initiated")]
            cluster_registration_token: self
                .to_cluster_registration_token(config.spec.cluster.clone()),
            config: config.clone(),
        })
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
