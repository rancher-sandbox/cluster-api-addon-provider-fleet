use crate::api::capi_cluster::Cluster;

use crate::api::fleet_addon_config::{ClusterConfig, FleetAddonConfig};
use crate::api::fleet_cluster::{self};

#[cfg(feature = "agent-initiated")]
use crate::api::fleet_cluster_registration_token::ClusterRegistrationToken;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::Error;
use cluster_api_rs::capi_cluster::ClusterTopology;
use fleet_api_rs::fleet_cluster::ClusterSpec;
use fleet_api_rs::fleet_clustergroup::{ClusterGroupSelector, ClusterGroupSpec};
use futures::channel::mpsc::Sender;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{Object, ObjectMeta, PatchParams, TypeMeta};

use kube::core::SelectorExt as _;
use kube::{api::ResourceExt, runtime::controller::Action, Resource};
use kube::{Api, Client};
#[cfg(feature = "agent-initiated")]
use rand::distr::{Alphanumeric, SampleString as _};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::warn;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use super::cluster_class::{CLUSTER_CLASS_LABEL, CLUSTER_CLASS_NAMESPACE_LABEL};
use super::controller::{
    fetch_config, get_or_create, patch, Context, FleetBundle, FleetController,
};
use super::{BundleResult, ClusterSyncError, ClusterSyncResult, LabelCheckResult};

pub static CONTROLPLANE_READY_CONDITION: &str = "ControlPlaneReady";

pub struct FleetClusterBundle {
    template_sources: TemplateSources,
    fleet: fleet_cluster::Cluster,
    fleet_group: Option<ClusterGroup>,
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
    fn to_group(self: &Cluster, config: Option<ClusterConfig>) -> Option<ClusterGroup> {
        if let Some(ClusterConfig {
            apply_class_group: Some(true),
            ..
        }) = config
        {
        } else {
            return None;
        };

        if let cluster_api_rs::capi_cluster::ClusterSpec {
            topology:
                Some(ClusterTopology {
                    class_namespace: Some(class_namespace),
                    class,
                    ..
                }),
            ..
        } = &self.spec
        {
            // Cluster groups creation for cluster class namespace are handled by ClusterClass controller
            if Some(class_namespace) == self.namespace().as_ref() {
                return None;
            }

            let labels = {
                let mut labels = BTreeMap::default();
                labels.insert(CLUSTER_CLASS_LABEL.to_string(), class.clone());
                labels.insert(
                    CLUSTER_CLASS_NAMESPACE_LABEL.to_string(),
                    class_namespace.clone(),
                );
                Some(labels)
            };

            return Some(ClusterGroup {
                types: Some(TypeMeta::resource::<ClusterGroup>()),
                metadata: ObjectMeta {
                    name: Some(format!("{class}.{class_namespace}")),
                    namespace: self.namespace(),
                    labels: labels.clone(),
                    owner_references: self.owner_ref(&()).into_iter().map(Into::into).collect(),
                    ..Default::default()
                },
                spec: ClusterGroupSpec {
                    selector: Some(ClusterGroupSelector {
                        match_labels: labels,
                        ..Default::default()
                    }),
                },
                ..Default::default()
            });
        }

        None
    }

    fn to_cluster(self: &Cluster, config: Option<ClusterConfig>) -> fleet_cluster::Cluster {
        let config = config.unwrap_or_default();
        let labels = match &self.spec.topology {
            Some(ClusterTopology {
                class,
                class_namespace,
                ..
            }) if !class.is_empty() => {
                let mut labels = self.labels().clone();
                labels.insert(CLUSTER_CLASS_LABEL.to_string(), class.clone());
                labels.insert(
                    CLUSTER_CLASS_NAMESPACE_LABEL.to_string(),
                    class_namespace
                        .clone()
                        .unwrap_or(self.namespace().unwrap_or_default()),
                );
                labels
            }
            None | Some(ClusterTopology { .. }) => self.labels().clone(),
        };

        fleet_cluster::Cluster {
            types: Some(TypeMeta::resource::<fleet_cluster::Cluster>()),
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
                true => ClusterSpec {
                    client_id: Some(Alphanumeric.sample_string(&mut rand::rng(), 64)),
                    agent_namespace: config.agent_install_namespace().into(),
                    agent_tolerations: config.agent_tolerations().into(),
                    host_network: config.host_network,
                    agent_env_vars: config.agent_env_vars,
                    ..Default::default()
                }
                .into(),
                false => ClusterSpec {
                    kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                    agent_namespace: config.agent_install_namespace().into(),
                    agent_tolerations: config.agent_tolerations().into(),
                    host_network: config.host_network,
                    agent_env_vars: config.agent_env_vars,
                    ..Default::default()
                }
                .into(),
            },
            #[cfg(not(feature = "agent-initiated"))]
            spec: ClusterSpec {
                kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                agent_namespace: config.agent_install_namespace().into(),
                agent_tolerations: config.agent_tolerations().into(),
                host_network: config.host_network,
                agent_env_vars: config.agent_env_vars,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[cfg(feature = "agent-initiated")]
    fn to_cluster_registration_token(
        self: &Cluster,
        config: Option<ClusterConfig>,
    ) -> Option<ClusterRegistrationToken> {
        use fleet_api_rs::fleet_cluster_registration_token::ClusterRegistrationTokenSpec;

        config?.agent_initiated?.then_some(true)?;

        ClusterRegistrationToken {
            metadata: self.into(),
            spec: ClusterRegistrationTokenSpec {
                ttl: Some("1h".into()),
            }
            .into(),
            ..Default::default()
        }
        .into()
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

        match self.config.cluster_patch_enabled() {
            true => {
                patch(
                    ctx.clone(),
                    cluster,
                    &PatchParams::apply("addon-provider-fleet"),
                )
                .await?
            }
            false => get_or_create(ctx.clone(), cluster).await?,
        };

        #[cfg(feature = "agent-initiated")]
        if let Some(cluster_registration_token) = self.cluster_registration_token.clone() {
            get_or_create(ctx.clone(), cluster_registration_token).await?;
        }

        if let Some(group) = self.fleet_group.clone() {
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
        let matching_labels = self.matching_labels(&config, ctx.client.clone()).await?;
        if !matching_labels || !config.cluster_operations_enabled() {
            return Ok(None);
        }

        if self.cluster_ready().is_none() {
            return Ok(None);
        }

        Ok(Some(FleetClusterBundle {
            template_sources: TemplateSources::new(self),
            fleet: self.to_cluster(config.spec.cluster.clone()),
            fleet_group: self.to_group(config.spec.cluster.clone()),
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
