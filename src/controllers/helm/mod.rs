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

pub mod install;
