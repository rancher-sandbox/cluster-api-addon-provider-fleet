use fleet_api_rs::fleet_cluster::{ClusterAgentEnvVars, ClusterAgentTolerations};
use k8s_openapi::{
    api::core::v1::ObjectReference,
    apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector},
};
use kube::{
    core::{ParseExpressionError, Selector},
    CELSchema, CustomResource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const AGENT_NAMESPACE: &str = "fleet-addon-agent";

/// This provides a config for fleet addon functionality
#[derive(CustomResource, Deserialize, Serialize, Clone, Default, Debug, CELSchema)]
#[kube(
    kind = "FleetAddonConfig",
    group = "addons.cluster.x-k8s.io",
    version = "v1alpha1",
    status = "FleetAddonConfigStatus",
    rule = Rule::new("self.metadata.name == 'fleet-addon-config'"),
)]
#[serde(rename_all = "camelCase")]
pub struct FleetAddonConfigSpec {
    /// Enable clusterClass controller functionality.
    ///
    /// This will create Fleet ClusterGroups for each ClusterClaster with the same name.
    pub cluster_class: Option<ClusterClassConfig>,

    /// Enable Cluster config funtionality.
    ///
    /// This will create Fleet Cluster for each Cluster with the same name.
    /// In case the cluster specifies topology.class, the name of the ClusterClass
    /// will be added to the Fleet Cluster labels.
    pub cluster: Option<ClusterConfig>,

    // Fleet chart configuratoin options
    pub config: Option<FleetConfig>,

    // Fleet chart installation options
    pub install: Option<FleetInstall>,
}

impl Default for FleetAddonConfig {
    fn default() -> Self {
        Self {
            metadata: Default::default(),
            spec: FleetAddonConfigSpec {
                cluster_class: Some(ClusterClassConfig::default()),
                cluster: Some(ClusterConfig::default()),
                config: Some(FleetConfig::default()),
                ..Default::default()
            },
            status: Default::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetAddonConfigStatus {
    pub installed_version: Option<String>,
    /// conditions represents the observations of a Fleet addon current state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterClassConfig {
    /// Setting to disable setting owner references on the created resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_owner_references: Option<bool>,

    /// Allow to patch resources, maintaining the desired state.
    /// If is not set, resources will only be re-created in case of removal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_resource: Option<bool>,
}

impl Default for ClusterClassConfig {
    fn default() -> Self {
        Self {
            patch_resource: Some(true),
            set_owner_references: Some(true),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterConfig {
    /// Apply a ClusterGroup for a ClusterClass referenced from a different namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_class_group: Option<bool>,

    /// Allow to patch resources, maintaining the desired state.
    /// If is not set, resources will only be re-created in case of removal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_resource: Option<bool>,

    /// Setting to disable setting owner references on the created resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_owner_references: Option<bool>,

    /// Naming settings for the fleet cluster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naming: Option<NamingStrategy>,

    /// Namespace selection for the fleet agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_namespace: Option<String>,

    /// Agent taint toleration settings for every cluster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_tolerations: Option<Vec<ClusterAgentTolerations>>,

    /// Host network allows to deploy agent configuration using hostNetwork: true setting
    /// which eludes dependency on the CNI configuration for the cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_network: Option<bool>,

    /// AgentEnvVars are extra environment variables to be added to the agent deployment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_env_vars: Option<Vec<ClusterAgentEnvVars>>,

    /// Import settings for the CAPI cluster. Allows to import clusters based on a set of labels,
    /// set on the cluster or the namespace.
    #[serde(flatten)]
    pub selectors: Selectors,

    #[cfg(feature = "agent-initiated")]
    /// Prepare initial cluster for agent initiated connection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_initiated: Option<bool>,
}

impl ClusterConfig {
    pub(crate) fn agent_install_namespace(&self) -> String {
        self.agent_namespace
            .clone()
            .unwrap_or(AGENT_NAMESPACE.to_string())
    }

    pub(crate) fn agent_tolerations(&self) -> Vec<ClusterAgentTolerations> {
        let agent_tolerations = vec![
            ClusterAgentTolerations {
                effect: Some("NoSchedule".into()),
                operator: Some("Exists".into()),
                key: Some("node.kubernetes.io/not-ready".into()),
                ..Default::default()
            },
            ClusterAgentTolerations {
                effect: Some("NoSchedule".into()),
                operator: Some("Exists".into()),
                key: Some("node.cluster.x-k8s.io/uninitialized".into()),
                ..Default::default()
            },
            ClusterAgentTolerations {
                effect: Some("NoSchedule".into()),
                operator: Some("Equal".into()),
                key: Some("node.cloudprovider.kubernetes.io/uninitialized".into()),
                value: Some("true".into()),
                ..Default::default()
            },
        ];

        self.agent_tolerations.clone().unwrap_or(agent_tolerations)
    }

    #[cfg(feature = "agent-initiated")]
    pub(crate) fn agent_initiated_connection(&self) -> bool {
        self.agent_initiated.filter(|&set| set).is_some()
    }

    pub(crate) fn apply_naming(&self, name: String) -> String {
        let strategy = self.naming.clone().unwrap_or_default();
        strategy.apply(name.clone().into()).unwrap_or(name)
    }

    pub(crate) fn apply_class_group(&self) -> bool {
        self.apply_class_group.is_some_and(|enabled| enabled)
    }
}

/// NamingStrategy is controlling Fleet cluster naming
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Default)]
pub struct NamingStrategy {
    /// Specify a prefix for the Cluster name, applied to created Fleet cluster
    pub prefix: Option<String>,
    /// Specify a suffix for the Cluster name, applied to created Fleet cluster
    pub suffix: Option<String>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            apply_class_group: Some(true),
            set_owner_references: Some(true),
            naming: Default::default(),
            agent_namespace: AGENT_NAMESPACE.to_string().into(),
            host_network: Some(true),
            #[cfg(feature = "agent-initiated")]
            agent_initiated: Some(true),
            selectors: Default::default(),
            patch_resource: Some(true),
            agent_env_vars: None,
            agent_tolerations: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetConfig {
    /// fleet server url configuration options
    pub server: Option<Server>,
    /// feature gates controlling experimental features
    pub feature_gates: Option<FeatureGates>,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            server: Default::default(),
            feature_gates: Some(FeatureGates::default()),
        }
    }
}

/// Feature toggles for enabling or disabling experimental functionality.
/// This struct controls access to specific experimental features.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGates {
    /// Enables experimental OCI  storage support.
    pub experimental_oci_storage: bool,

    /// Enables experimental Helm operations support.
    pub experimental_helm_ops: bool,
}

impl Default for FeatureGates {
    fn default() -> Self {
        Self {
            // Unless is set otherwise, these features are enabled by CAAPF
            experimental_oci_storage: true,
            experimental_helm_ops: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetInstall {
    /// Chart version to install
    #[serde(flatten)]
    pub install_version: Install,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Install {
    /// Follow the latest version of the chart on install
    FollowLatest(bool),

    /// Use specific version to install
    Version(String),
}

impl Install {
    /// Perform version normalization for comparison with `helm search` app_version output
    pub(crate) fn normalized(self) -> Self {
        match self {
            Install::FollowLatest(_) => self,
            Install::Version(version) => {
                Install::Version(version.strip_prefix("v").unwrap_or(&version).into())
            }
        }
    }
}

impl Default for Install {
    fn default() -> Self {
        Self::FollowLatest(true)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Server {
    InferLocal(bool),
    Custom(InstallOptions),
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstallOptions {
    pub api_server_ca_config_ref: Option<ObjectReference>,
    pub api_server_url: Option<String>,
}

impl NamingStrategy {
    pub fn apply(&self, name: Option<String>) -> Option<String> {
        name.map(|name| match &self.prefix {
            Some(prefix) => prefix.clone() + &name,
            None => name,
        })
        .map(|name| match &self.suffix {
            Some(suffix) => name + suffix,
            None => name,
        })
    }
}

/// Selectors is controlling Fleet import strategy settings.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct Selectors {
    /// Namespace label selector. If set, only clusters in the namespace matching label selector will be imported.
    pub namespace_selector: LabelSelector,

    /// Cluster label selector. If set, only clusters matching label selector will be imported.
    pub selector: LabelSelector,
}

impl FleetAddonConfig {
    // Provide a static label selector for cluster objects, which can be always be set
    // and will not cause cache events from resources in the labeled Namespace to be missed
    pub(crate) fn cluster_watch(&self) -> Result<Selector, ParseExpressionError> {
        Ok(self
            .namespace_selector()?
            .selects_all()
            .then_some(self.cluster_selector()?)
            .unwrap_or_default())
    }

    // Raw cluster selector
    pub(crate) fn cluster_selector(&self) -> Result<Selector, ParseExpressionError> {
        self.spec
            .cluster
            .as_ref()
            .map(|c| c.selectors.selector.clone())
            .unwrap_or_default()
            .try_into()
    }

    // Raw namespace selector
    pub(crate) fn namespace_selector(&self) -> Result<Selector, ParseExpressionError> {
        self.spec
            .cluster
            .as_ref()
            .map(|c| c.selectors.namespace_selector.clone())
            .unwrap_or_default()
            .try_into()
    }

    // Check for general cluster operations, like create, patch, etc. Evaluates to false if disabled.
    pub(crate) fn cluster_operations_enabled(&self) -> bool {
        self.spec.cluster.is_some()
    }

    // Check for general ClusterClass operations, like create, patch, etc. Evaluates to false if disabled.
    pub(crate) fn cluster_class_operations_enabled(&self) -> bool {
        self.spec.cluster_class.is_some()
    }

    // Check for general cluster patching setting.
    pub(crate) fn cluster_patch_enabled(&self) -> bool {
        self.spec
            .cluster
            .as_ref()
            .map(|c| c.patch_resource)
            .unwrap_or_default()
            .filter(|&enabled| enabled)
            .is_some()
    }

    // Check for general clusterClass patching setting.
    pub(crate) fn cluster_class_patch_enabled(&self) -> bool {
        self.spec
            .cluster_class
            .as_ref()
            .map(|c| c.patch_resource)
            .unwrap_or_default()
            .filter(|&enabled| enabled)
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::fleet_addon_config::NamingStrategy;

    #[tokio::test]
    async fn test_naming_strategy() {
        assert_eq!(
            Some("prefixtestsuffix".to_string()),
            NamingStrategy {
                prefix: "prefix".to_string().into(),
                suffix: "suffix".to_string().into(),
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            Some("testsuffix".to_string()),
            NamingStrategy {
                suffix: "suffix".to_string().into(),
                ..Default::default()
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            Some("prefixtest".to_string()),
            NamingStrategy {
                prefix: "prefix".to_string().into(),
                ..Default::default()
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            Some("test".to_string()),
            NamingStrategy {
                ..Default::default()
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            None,
            NamingStrategy {
                prefix: "prefix".to_string().into(),
                suffix: "suffix".to_string().into(),
            }
            .apply(None)
        );
    }
}
