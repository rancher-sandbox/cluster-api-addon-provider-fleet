#[allow(unused_imports)]
mod prelude {
    pub use kube::CustomResource;
    pub use schemars::JsonSchema;
    pub use serde::{Deserialize, Serialize};
    pub use std::collections::BTreeMap;
}
use fleet_api_rs::fleet_clustergroup::{ClusterGroupSpec, ClusterGroupStatus};

use self::prelude::*;

#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
#[kube(
    group = "fleet.cattle.io",
    version = "v1alpha1",
    kind = "ClusterGroup",
    plural = "clustergroups"
)]
#[kube(namespaced)]
#[kube(status = "ClusterGroupStatus")]
#[kube(derive = "Default")]
pub struct ClusterGroupProxy {
    #[serde(flatten)]
    pub proxy: ClusterGroupSpec,
}

impl From<ClusterGroupSpec> for ClusterGroupProxy {
    fn from(val: ClusterGroupSpec) -> Self {
        ClusterGroupProxy { proxy: val }
    }
}
