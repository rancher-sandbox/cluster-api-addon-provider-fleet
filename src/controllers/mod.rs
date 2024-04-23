use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Cluster sync error: {0}")]
    ClusterSync(#[source] GetOrCreateError),

    #[error("Cluster group sync error: {0}")]
    GroupSync(#[source] GetOrCreateError),

    #[error("Return early")]
    EarlyReturn,
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

pub mod cluster;
pub mod cluster_class;
pub mod controller;
