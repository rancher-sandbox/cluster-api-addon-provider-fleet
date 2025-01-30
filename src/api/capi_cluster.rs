use cluster_api_rs::capi_cluster::{ClusterSpec, ClusterStatus};
use kube::{api::{ObjectMeta, TypeMeta}, Resource};
use serde::{Deserialize, Serialize};

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = cluster_api_rs::capi_cluster::Cluster)]
pub struct Cluster {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterSpec,
    pub status: Option<ClusterStatus>,
}
