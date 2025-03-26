use fleet_api_rs::fleet_bundle_namespace_mapping::{
    BundleNamespaceMappingBundleSelector, BundleNamespaceMappingNamespaceSelector,
};
use kube::{
    api::{ObjectMeta, TypeMeta},
    Resource,
};
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
