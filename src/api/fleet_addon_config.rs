use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::{
    core::{ParseExpressionError, Selector},
    CustomResource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const AGENT_NAMESPACE: &str = "fleet-addon-agent";

/// This provides a config for fleet addon functionality
#[derive(CustomResource, Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[kube(
    kind = "FleetAddonConfig",
    group = "addons.cluster.x-k8s.io",
    version = "v1alpha1"
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
}

impl Default for FleetAddonConfig {
    fn default() -> Self {
        Self {
            metadata: Default::default(),
            spec: FleetAddonConfigSpec {
                cluster_class: Some(ClusterClassConfig::default()),
                cluster: Some(ClusterConfig::default()),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterClassConfig {
    /// Setting to disable setting owner references on the created resources
    pub set_owner_references: Option<bool>,

    /// Allow to patch resources, maintaining the desired state.
    /// If is not set, resources will only be re-created in case of removal.
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
    /// Allow to patch resources, maintaining the desired state.
    /// If is not set, resources will only be re-created in case of removal.
    pub patch_resource: Option<bool>,

    /// Setting to disable setting owner references on the created resources
    pub set_owner_references: Option<bool>,

    /// Naming settings for the fleet cluster
    pub naming: Option<NamingStrategy>,

    /// Namespace selection for the fleet agent
    pub agent_namespace: Option<String>,

    /// Host network allows to deploy agent configuration using hostNetwork: true setting
    /// which eludes dependency on the CNI configuration for the cluster.
    pub host_network: Option<bool>,

    /// Import settings for the CAPI cluster. Allows to import clusters based on a set of labels,
    /// set on the cluster or the namespace.
    #[serde(flatten)]
    pub selectors: Selectors,

    // /// Cluster label selector. If set, only clusters matching label selector will be imported.
    // /// WARN: this field controls the state of opened watches to the cluster. If changed, requires controller to be reloaded.
    // pub cluster: Option<LabelSelector>,
    #[cfg(feature = "agent-initiated")]
    /// Prepare initial cluster for agent initiated connection
    pub agent_initiated: Option<bool>,
}

impl ClusterConfig {
    pub(crate) fn agent_install_namespace(&self) -> String {
        self.agent_namespace
            .clone()
            .unwrap_or(AGENT_NAMESPACE.to_string())
    }

    #[cfg(feature = "agent-initiated")]
    pub(crate) fn agent_initiated_connection(&self) -> bool {
        self.agent_initiated.filter(|&set| set).is_some()
    }

    pub(crate) fn apply_naming(&self, name: String) -> String {
        let strategy = self.naming.clone().unwrap_or_default();
        strategy.apply(name.clone().into()).unwrap_or(name)
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
            set_owner_references: Some(true),
            naming: Default::default(),
            agent_namespace: AGENT_NAMESPACE.to_string().into(),
            host_network: Some(true),
            #[cfg(feature = "agent-initiated")]
            agent_initiated: Some(true),
            selectors: Default::default(),
            patch_resource: Some(true),
        }
    }
}

impl NamingStrategy {
    pub fn apply(&self, name: Option<String>) -> Option<String> {
        name.map(|name| match &self.prefix {
            Some(prefix) => prefix.clone() + &name,
            None => name,
        })
        .map(|name| match &self.suffix {
            Some(suffix) => name + &suffix,
            None => name,
        })
    }
}

/// Selectors is controlling Fleet import strategy settings.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct Selectors {
    /// Namespace label selector. If set, only clusters in the namespace matching label selector will be imported.
    /// WARN: this field controls the state of opened watches to the cluster. If changed, requires controller to be reloaded.
    pub namespace_selector: LabelSelector,

    /// Cluster label selector. If set, only clusters matching label selector will be imported.
    /// WARN: this field controls the state of opened watches to the cluster. If changed, requires controller to be reloaded.
    pub selector: LabelSelector,
}

impl FleetAddonConfig {
    // Raw cluster selector
    pub(crate) fn cluster_selector(&self) -> Result<Selector, ParseExpressionError> {
        self.spec
            .cluster
            .as_ref()
            .map(|c| c.selectors.selector.clone())
            .unwrap_or_default()
            .try_into()
    }

    // Provide a static label selector for cluster objects, which can be always be set
    // and will not cause cache events from resources in the labeled Namespace to be missed
    pub(crate) fn cluster_watch(&self) -> Result<Selector, ParseExpressionError> {
        Ok(self
            .namespace_selector()?
            .selects_all()
            .then_some(self.cluster_selector()?)
            .unwrap_or_default())
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
            .cluster
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
