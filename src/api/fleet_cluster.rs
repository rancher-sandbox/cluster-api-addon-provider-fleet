#[allow(unused_imports)]
mod prelude {
    pub use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
    pub use kube::CustomResource;
    pub use schemars::JsonSchema;
    pub use serde::{Deserialize, Serialize};
    pub use std::collections::BTreeMap;
}
use fleet_api_rs::fleet_cluster::{ClusterSpec, ClusterStatus};

use self::prelude::*;

#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
#[kube(
    group = "fleet.cattle.io",
    version = "v1alpha1",
    kind = "Cluster",
    plural = "clusters"
)]
#[kube(namespaced)]
#[kube(status = "ClusterStatus")]
#[kube(derive = "Default")]
pub struct ClusterSpecProxy {
    #[serde(flatten)]
    pub proxy: ClusterSpec,
}

impl From<ClusterSpec> for ClusterSpecProxy {
    fn from(val: ClusterSpec) -> Self {
        ClusterSpecProxy { proxy: val }
    }
}
