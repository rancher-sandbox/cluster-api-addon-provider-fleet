use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::{core::Selector, CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// This provides a config for fleet addon functionality
#[derive(CustomResource, Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[kube(
    kind = "FleetAddonConfig",
    group = "addons.cluster.x-k8s.io",
    version = "v1alpha1"
)]
pub struct FleetAddonConfigSpec {
    /// Allow to patch resources, maintaining the desired state.
    pub patch_resource: Option<bool>,
    /// Import settings for the CAPI cluster. Allows to import clusters based on a set of labels,
    /// set on the cluster or the namespace.
    pub selectors: Option<Selectors>,
    /// Cluster class controller settings
    pub cluster_class: Option<ClusterClassConfig>,
    /// Cluster controller settings
    pub cluster: Option<ClusterConfig>,
}

impl Default for FleetAddonConfig {
    fn default() -> Self {
        Self {
            metadata: Default::default(),
            spec: FleetAddonConfigSpec {
                patch_resource: Some(true),
                cluster_class: Some(ClusterClassConfig::default()),
                selectors: None,
                cluster: Some(ClusterConfig::default()),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct ClusterClassConfig {
    /// Enable clusterClass controller functionality.
    ///
    /// This will create Fleet ClusterGroups for each ClusterClaster with the same name.
    pub enabled: Option<bool>,

    /// Setting to disable setting owner references on the created resources
    pub set_owner_references: Option<bool>,
}

impl Default for ClusterClassConfig {
    fn default() -> Self {
        Self {
            set_owner_references: Some(true),
            enabled: Some(true),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct ClusterConfig {
    /// Enable Cluster config funtionality.
    ///
    /// This will create Fleet Cluster for each Cluster with the same name.
    /// In case the cluster specifies topology.class, the name of the ClusterClass
    /// will be added to the Fleet Cluster labels.
    pub enabled: Option<bool>,

    /// Setting to disable setting owner references on the created resources
    pub set_owner_references: Option<bool>,

    /// Naming settings for the fleet cluster
    pub naming: NamingStrategy,

    /// Namespace selection for the fleet agent
    pub agent_namespace: Option<String>,

    #[cfg(feature = "agent-initiated")]
    /// Prepare initial cluster for agent initiated connection
    pub agent_initiated: Option<bool>,
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
            agent_namespace: "fleet-addon-agent".to_string().into(),
            enabled: Some(true),
            #[cfg(feature = "agent-initiated")]
            agent_initiated: Some(true),
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
pub struct Selectors {
    /// Namespace label selector. If set, only clusters in the namespace matching label selector will be imported.
    /// WARN: this field controls the state of opened watches to the cluster. If changed, requires controller to be reloaded.
    pub namespace: LabelSelector,

    /// Cluster label selector. If set, only clusters matching label selector will be imported.
    /// WARN: this field controls the state of opened watches to the cluster. If changed, requires controller to be reloaded.
    pub cluster: LabelSelector,
}

impl FleetAddonConfig {
    // Raw cluster selector
    pub(crate) fn cluster_selector(&self) -> Selector {
        self.spec
            .selectors
            .clone()
            .unwrap_or_default()
            .cluster
            .into()
    }

    // Provide a static label selector for cluster objects, which can be always be set
    // and will not cause cache events from resources in the labeled Namespace to be missed
    pub(crate) fn cluster_watch(&self) -> Selector {
        self.namespace_selector()
            .selects_all()
            .then_some(self.cluster_selector())
            .unwrap_or_default()
    }

    // Raw namespace selector
    pub(crate) fn namespace_selector(&self) -> Selector {
        self.spec
            .selectors
            .clone()
            .unwrap_or_default()
            .namespace
            .into()
    }

    // Check for general cluster operations, like create, patch, etc. Evaluates to false if disabled.
    pub(crate) fn cluster_operations_enabled(&self) -> bool {
        self.spec
            .cluster
            .iter()
            .filter_map(|c| c.enabled)
            .find(|&enabled| enabled)
            .is_some()
    }

    // Check for general ClusterClass operations, like create, patch, etc. Evaluates to false if disabled.
    pub(crate) fn cluster_class_operations_enabled(&self) -> bool {
        self.spec
            .cluster_class
            .iter()
            .filter_map(|c| c.enabled)
            .find(|&enabled| enabled)
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
