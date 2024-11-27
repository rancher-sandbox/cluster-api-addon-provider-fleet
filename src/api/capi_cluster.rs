#[allow(unused_imports)]
mod prelude {
    pub use k8s_openapi::api::core::v1::ObjectReference;
    pub use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
    pub use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
    pub use kube::CustomResource;
    pub use schemars::JsonSchema;
    pub use serde::{Deserialize, Serialize};
    pub use std::collections::BTreeMap;
}
use cluster_api_rs::capi_cluster::{ClusterSpec, ClusterStatus};

use self::prelude::*;

/// ClusterProxy defines the desired state of the CAPI Cluster.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "cluster.x-k8s.io",
    version = "v1beta1",
    kind = "Cluster",
    plural = "clusters"
)]
#[kube(namespaced)]
#[kube(status = "ClusterStatus")]
pub struct ClusterProxy {
    #[serde(flatten)]
    pub proxy: ClusterSpec,
}
