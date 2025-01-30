use serde::{Deserialize, Serialize};
use fleet_api_rs::fleet_clustergroup::{ClusterGroupSpec, ClusterGroupStatus};
use kube::{api::{ObjectMeta, TypeMeta}, Resource};

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = fleet_api_rs::fleet_clustergroup::ClusterGroup)]
pub struct ClusterGroup {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterGroupSpec,
    pub status: Option<ClusterGroupStatus>,
}
