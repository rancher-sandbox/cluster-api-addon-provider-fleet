use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("{0}")]
    ClusterSync(#[from] ClusterSyncError),

    #[error("{0}")]
    GroupSync(#[from] GroupSyncError),

    #[error("{0}")]
    LabelCheck(#[from] LabelCheckError),

    #[error("Cluster registration token create error {0}")]
    ClusterRegistrationTokenSync(#[from] GetOrCreateError),

    #[error("Return early")]
    EarlyReturn,
}

#[derive(Error, Debug)]
pub enum ClusterSyncError {
    #[error("Cluster create error: {0}")]
    GetOrCreateError(#[from] GetOrCreateError),

    #[error("Cluster update error: {0}")]
    PatchError(#[from] PatchError),
}

#[derive(Error, Debug)]
pub enum GroupSyncError {
    #[error("Cluster group create error: {0}")]
    GetOrCreateError(#[from] GetOrCreateError),

    #[error("Cluster group update error: {0}")]
    PatchError(#[from] PatchError),
}

#[derive(Error, Debug)]
pub enum GetOrCreateError {
    #[error("Lookup error: {0}")]
    Lookup(#[source] kube::Error),

    #[error("Create error: {0}")]
    Create(#[source] kube::Error),

    #[error("Diagnostics error: {0}")]
    Event(#[from] kube::Error),
}

#[derive(Error, Debug)]
pub enum LabelCheckError {
    #[error("Namespace lookup error: {0}")]
    NamespaceLookup(#[from] kube::Error),

    #[error("Parse expression error: {0}")]
    Expression(#[from] kube::core::ParseExpressionError),
}

#[derive(Error, Debug)]
pub enum PatchError {
    #[error("Patch error: {0}")]
    Patch(#[source] kube::Error),

    #[error("Diagnostics error: {0}")]
    Event(#[from] kube::Error),
}

pub mod cluster;
pub mod cluster_class;
pub mod cluster_group;
pub mod controller;
