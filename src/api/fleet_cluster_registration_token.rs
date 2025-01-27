#[allow(unused_imports)]
mod prelude {
    pub use kube::CustomResource;
    pub use schemars::JsonSchema;
    pub use serde::{Deserialize, Serialize};
}
use fleet_api_rs::fleet_cluster_registration_token::{
    ClusterRegistrationTokenSpec, ClusterRegistrationTokenStatus,
};

use self::prelude::*;

#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
#[kube(
    group = "fleet.cattle.io",
    version = "v1alpha1",
    kind = "ClusterRegistrationToken",
    plural = "clusterregistrationtokens"
)]
#[kube(namespaced)]
#[kube(status = "ClusterRegistrationTokenStatus")]
#[kube(derive = "Default")]
pub struct ClusterRegistrationTokenProxy {
    #[serde(flatten)]
    pub proxy: ClusterRegistrationTokenSpec,
}

impl Into<ClusterRegistrationTokenProxy> for ClusterRegistrationTokenSpec {
    fn into(self) -> ClusterRegistrationTokenProxy {
        ClusterRegistrationTokenProxy { proxy: self }
    }
}
