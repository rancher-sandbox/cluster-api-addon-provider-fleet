use crate::api::capi_cluster::{Cluster, ClusterTopology};

use crate::api::fleet_addon_config::{ClusterConfig, FleetAddonConfig};
use crate::api::fleet_cluster::{self, ClusterAgentTolerations};

#[cfg(feature = "agent-initiated")]
use crate::api::fleet_cluster_registration_token::{
    ClusterRegistrationToken, ClusterRegistrationTokenSpec,
};
use crate::{Error, Result};
use futures::channel::mpsc::Sender;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::ObjectMeta;

use kube::core::SelectorExt as _;
use kube::{api::ResourceExt, runtime::controller::Action, Resource};
use kube::{Api, Client};
#[cfg(feature = "agent-initiated")]
use rand::distributions::{Alphanumeric, DistString as _};
use tokio::sync::Mutex;
use tracing::warn;

use std::sync::Arc;
use std::time::Duration;

use super::cluster_class::CLUSTER_CLASS_LABEL;
use super::controller::{get_or_create, patch, Context, FleetBundle, FleetController};
use super::{ClusterSyncError, LabelCheckError, SyncError};

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

        let agent_tolerations = Some(vec![ClusterAgentTolerations{
            effect: Some("NoSchedule".into()),
            operator: Some("Equal".into()),
            key: Some("node.kubernetes.io/not-ready".into()),
            ..Default::default()
        }]);

        fleet_cluster::Cluster {
            metadata: ObjectMeta {
                labels: Some(labels),
                owner_references: config
                    .set_owner_references
                    .is_some_and(|set| set)
                    .then_some(self.owner_ref(&()).into_iter().collect()),
                name: config.apply_naming(self.name_any()).into(),
                ..self.into()
            },
            #[cfg(feature = "agent-initiated")]
            spec: match config.agent_initiated_connection() {
                true => fleet_cluster::ClusterSpec {
                    client_id: Some(Alphanumeric.sample_string(&mut rand::thread_rng(), 64)),
                    agent_namespace: config.agent_install_namespace().into(),
                    host_network: config.host_network,
                    agent_tolerations,
                    ..Default::default()
                },
                false => fleet_cluster::ClusterSpec {
                    kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                    agent_namespace: config.agent_install_namespace().into(),
                    host_network: config.host_network,
                    agent_tolerations,
                    ..Default::default()
                },
            },
            #[cfg(not(feature = "agent-initiated"))]
            spec: fleet_cluster::ClusterSpec {
                kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                agent_namespace: config.agent_install_namespace().into(),
                host_network: config.host_network,
                agent_tolerations,
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

        if self.config.cluster_patch_enabled() {
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

    async fn to_bundle(
        &self,
        ctx: Arc<Context>,
        config: &FleetAddonConfig,
    ) -> Result<FleetClusterBundle> {
        let matching_labels = self
            .matching_labels(config, ctx.client.clone())
            .await
            .map_err(Into::<SyncError>::into)?;

        if !matching_labels || !config.cluster_operations_enabled() {
            Err(SyncError::EarlyReturn)?;
        }

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

    pub async fn matching_labels(
        &self,
        config: &FleetAddonConfig,
        client: Client,
    ) -> Result<bool, LabelCheckError> {
        let matches = config.cluster_selector()?.matches(self.labels()) || {
            let ns = self.namespace().unwrap_or("default".into());
            let namespace: Namespace = Api::all(client).get(ns.as_str()).await?;
            config.namespace_selector()?.matches(namespace.labels())
        };

        Ok(matches)
    }

    pub async fn reconcile_ns(
        _: Arc<Namespace>,
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
