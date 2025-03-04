use fleet_api_rs::fleet_cluster_registration_token::{
    ClusterRegistrationTokenSpec, ClusterRegistrationTokenStatus,
};
use kube::{
    api::{ObjectMeta, TypeMeta},
    Resource,
};
use serde::{Deserialize, Serialize};

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = fleet_api_rs::fleet_cluster_registration_token::ClusterRegistrationToken)]
pub struct ClusterRegistrationToken {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterRegistrationTokenSpec,
    pub status: Option<ClusterRegistrationTokenStatus>,
}
