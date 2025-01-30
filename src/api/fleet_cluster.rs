use serde::{Deserialize, Serialize};
use fleet_api_rs::fleet_cluster::{ClusterSpec, ClusterStatus};
use kube::{api::{ObjectMeta, TypeMeta}, Resource};

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = fleet_api_rs::fleet_cluster::Cluster)]
pub struct Cluster {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterSpec,
    pub status: Option<ClusterStatus>,
}
