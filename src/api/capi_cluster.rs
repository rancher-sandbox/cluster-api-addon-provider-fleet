use std::collections::BTreeMap;

use cluster_api_rs::capi_cluster::{ClusterSpec, ClusterStatus, ClusterTopology};
use fleet_api_rs::fleet_clustergroup::{ClusterGroupSelector, ClusterGroupSpec};
use kube::{
    api::{ObjectMeta, TypeMeta},
    Resource, ResourceExt as _,
};
#[cfg(feature = "agent-initiated")]
use rand::distr::{Alphanumeric, SampleString as _};
use serde::{Deserialize, Serialize};

use super::{
    bundle_namespace_mapping::{BundleNamespaceMapping, BundleNamespaceMappingNamespaceSelector},
    fleet_addon_config::ClusterConfig,
    fleet_cluster,
    fleet_clustergroup::{ClusterGroup, CLUSTER_CLASS_LABEL, CLUSTER_CLASS_NAMESPACE_LABEL},
};

#[cfg(feature = "agent-initiated")]
use super::fleet_cluster_registration_token::ClusterRegistrationToken;

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = cluster_api_rs::capi_cluster::Cluster)]
pub struct Cluster {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterSpec,
    pub status: Option<ClusterStatus>,
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
    pub(crate) fn to_group(self: &Cluster, config: Option<&ClusterConfig>) -> Option<ClusterGroup> {
        config?.apply_class_group().then_some(true)?;

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

    pub(crate) fn to_cluster(
        self: &Cluster,
        config: Option<&ClusterConfig>,
    ) -> fleet_cluster::Cluster {
        let empty = ClusterConfig::default();
        let config = config.unwrap_or(&empty);
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
                true => fleet_api_rs::fleet_cluster::ClusterSpec {
                    client_id: Some(Alphanumeric.sample_string(&mut rand::rng(), 64)),
                    agent_namespace: config.agent_install_namespace().into(),
                    agent_tolerations: config.agent_tolerations().into(),
                    host_network: config.host_network,
                    agent_env_vars: config.agent_env_vars.clone(),
                    ..Default::default()
                }
                .into(),
                false => fleet_api_rs::fleet_cluster::ClusterSpec {
                    kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                    agent_namespace: config.agent_install_namespace().into(),
                    agent_tolerations: config.agent_tolerations().into(),
                    host_network: config.host_network,
                    agent_env_vars: config.agent_env_vars.clone(),
                    ..Default::default()
                }
                .into(),
            },
            #[cfg(not(feature = "agent-initiated"))]
            spec: fleet_api_rs::fleet_cluster::ClusterSpec {
                kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                agent_namespace: config.agent_install_namespace().into(),
                agent_tolerations: config.agent_tolerations().into(),
                host_network: config.host_network,
                agent_env_vars: config.agent_env_vars.clone(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub(crate) fn to_bundle_ns_mapping(
        &self,
        config: Option<&ClusterConfig>,
    ) -> Option<BundleNamespaceMapping> {
        config?.apply_class_group().then_some(true)?;

        let topology = self.spec.topology.clone()?;
        let class_namespace = topology.class_namespace?;

        let match_labels = {
            let mut labels = BTreeMap::default();
            labels.insert("kubernetes.io/metadata.name".into(), self.namespace()?);
            Some(labels)
        };

        Some(BundleNamespaceMapping {
            types: Some(TypeMeta::resource::<BundleNamespaceMapping>()),
            metadata: ObjectMeta {
                name: self.namespace(),
                namespace: Some(class_namespace),
                ..Default::default()
            },
            bundle_selector: Default::default(),
            namespace_selector: BundleNamespaceMappingNamespaceSelector {
                match_labels,
                ..Default::default()
            },
        })
    }

    #[cfg(feature = "agent-initiated")]
    pub(crate) fn to_cluster_registration_token(
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
