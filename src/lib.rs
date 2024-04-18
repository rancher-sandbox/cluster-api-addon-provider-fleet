use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("Fleet cluster lookup error: {0}")]
    FleetClusterLookupError(#[source] kube::Error),

    #[error("Fleet cluster create error: {0}")]
    FleetClusterCreateError(#[source] kube::Error),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("Missing cluster namespace")]
    ClusterNamespaceMissing,

    #[error("IllegalDocument")]
    IllegalDocument,

    #[error("Return early")]
    EarlyReturn,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

/// Expose all controller components used by main
pub mod controller;
pub use crate::controller::*;
pub mod api;

/// Log and trace integrations
pub mod telemetry;

/// Metrics
mod metrics;
pub use metrics::Metrics;

/*
#[cfg(test)] pub mod fixtures;
*/
