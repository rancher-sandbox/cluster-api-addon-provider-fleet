use kube::CustomResource;
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

    /// Specifies a name suffix for the fleet cluster
    pub naming: NamingStrategy,

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
