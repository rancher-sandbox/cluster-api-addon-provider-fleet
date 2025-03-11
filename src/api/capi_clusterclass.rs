use cluster_api_rs::capi_clusterclass::{ClusterClassSpec, ClusterClassStatus};
use kube::{
    api::{ObjectMeta, TypeMeta},
    Resource,
};
use serde::{Deserialize, Serialize};

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = cluster_api_rs::capi_clusterclass::ClusterClass)]
pub struct ClusterClass {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterClassSpec,
    pub status: Option<ClusterClassStatus>,
}
