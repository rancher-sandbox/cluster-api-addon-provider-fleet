use std::collections::BTreeMap;

use kube::{
    api::{ObjectMeta, TypeMeta},
    Resource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod mapping {
    use kube::CustomResource;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(CustomResource, Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
    #[kube(
        kind = "BundleNamespaceMapping",
        group = "fleet.cattle.io",
        version = "v1alpha1",
        namespaced
    )]
    pub struct BundleNamespaceMappingFantomSpec {}
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = mapping::BundleNamespaceMapping)]
#[serde(rename_all = "camelCase")]
pub struct BundleNamespaceMapping {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub bundle_selector: BundleNamespaceMappingBundleSelector,
    pub namespace_selector: BundleNamespaceMappingNamespaceSelector,
}

/// A label selector is a label query over a set of resources. The result of matchLabels and
/// matchExpressions are ANDed. An empty label selector matches all objects. A null
/// label selector matches no objects.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct BundleNamespaceMappingBundleSelector {
    /// matchExpressions is a list of label selector requirements. The requirements are ANDed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "matchExpressions"
    )]
    pub match_expressions: Option<Vec<BundleNamespaceMappingBundleSelectorMatchExpressions>>,
    /// matchLabels is a map of {key,value} pairs. A single {key,value} in the matchLabels
    /// map is equivalent to an element of matchExpressions, whose key field is "key", the
    /// operator is "In", and the values array contains only "value". The requirements are ANDed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "matchLabels"
    )]
    pub match_labels: Option<BTreeMap<String, String>>,
}

/// A label selector requirement is a selector that contains values, a key, and an operator that
/// relates the key and values.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct BundleNamespaceMappingBundleSelectorMatchExpressions {
    /// key is the label key that the selector applies to.
    pub key: String,
    /// operator represents a key's relationship to a set of values.
    /// Valid operators are In, NotIn, Exists and DoesNotExist.
    pub operator: String,
    /// values is an array of string values. If the operator is In or NotIn,
    /// the values array must be non-empty. If the operator is Exists or DoesNotExist,
    /// the values array must be empty. This array is replaced during a strategic
    /// merge patch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

/// A label selector is a label query over a set of resources. The result of matchLabels and
/// matchExpressions are ANDed. An empty label selector matches all objects. A null
/// label selector matches no objects.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct BundleNamespaceMappingNamespaceSelector {
    /// matchExpressions is a list of label selector requirements. The requirements are ANDed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "matchExpressions"
    )]
    pub match_expressions: Option<Vec<BundleNamespaceMappingNamespaceSelectorMatchExpressions>>,
    /// matchLabels is a map of {key,value} pairs. A single {key,value} in the matchLabels
    /// map is equivalent to an element of matchExpressions, whose key field is "key", the
    /// operator is "In", and the values array contains only "value". The requirements are ANDed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "matchLabels"
    )]
    pub match_labels: Option<BTreeMap<String, String>>,
}

/// A label selector requirement is a selector that contains values, a key, and an operator that
/// relates the key and values.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct BundleNamespaceMappingNamespaceSelectorMatchExpressions {
    /// key is the label key that the selector applies to.
    pub key: String,
    /// operator represents a key's relationship to a set of values.
    /// Valid operators are In, NotIn, Exists and DoesNotExist.
    pub operator: String,
    /// values is an array of string values. If the operator is In or NotIn,
    /// the values array must be non-empty. If the operator is Exists or DoesNotExist,
    /// the values array must be empty. This array is replaced during a strategic
    /// merge patch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}
