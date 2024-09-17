use std::io;

use thiserror::Error;

pub type FleetInstallResult<T> = std::result::Result<T, FleetInstallError>;

#[derive(Error, Debug)]
pub enum FleetInstallError {
    #[error("Fleet install error: {0}")]
    FleetInstall(#[from] io::Error),
}

pub type FleetCRDInstallResult<T> = std::result::Result<T, FleetCRDInstallError>;

#[derive(Error, Debug)]
pub enum FleetCRDInstallError {
    #[error("CRD install error: {0}")]
    CRDInstall(#[from] io::Error),
}

pub type RepoAddResult<T> = std::result::Result<T, RepoAddError>;

#[derive(Error, Debug)]
pub enum RepoAddError {
    #[error("Fleet repo add error: {0}")]
    RepoAdd(#[from] io::Error),
}

pub type RepoUpdateResult<T> = std::result::Result<T, RepoUpdateError>;

#[derive(Error, Debug)]
pub enum RepoUpdateError {
    #[error("Fleet repo update error: {0}")]
    RepoUpdate(#[from] io::Error),
}

pub type RepoSearchResult<T> = std::result::Result<T, RepoSearchError>;

#[derive(Error, Debug)]
pub enum RepoSearchError {
    #[error("Fleet repo search error: {0}")]
    RepoSearch(#[from] io::Error),

    #[error("Decode error: {0}")]
    UTF8Error(#[from] std::string::FromUtf8Error),

    #[error("Deserialize search error: {0}")]
    DeserializeInfoError(#[from] serde_json::Error),
}

pub type MetadataGetResult<T> = std::result::Result<T, MetadataGetError>;

#[derive(Error, Debug)]
pub enum MetadataGetError {
    #[error("Metadata get error: {0}")]
    MetadataGet(#[from] io::Error),

    #[error("Decode error: {0}")]
    UTF8Error(#[from] std::string::FromUtf8Error),

    #[error("Deserialize info error: {0}")]
    DeserializeInfoError(#[from] serde_json::Error),
}

pub mod install;
