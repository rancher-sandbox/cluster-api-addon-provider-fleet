use std::process::{Child, Command, Stdio};

use serde::Deserialize;

use crate::api::fleet_addon_config::Install;

use super::{
    FleetCRDInstallResult, FleetInstallResult, MetadataGetResult, RepoAddResult, RepoSearchResult,
    RepoUpdateResult,
};

#[derive(Default)]
pub struct FleetChart {
    pub repo: String,
    pub version: Install,
    pub namespace: String,

    pub wait: bool,
    pub update_dependency: bool,
    pub create_namespace: bool,
    pub bootstrap_local_cluster: bool,
    pub experimental_oci_ops: bool,
}

#[derive(Deserialize)]
pub struct ChartInfo {
    pub name: String,
    pub namespace: String,
    pub app_version: String,
    pub status: String,
}

#[derive(Deserialize)]
pub struct ChartSearch {
    pub name: String,
    pub app_version: String,
}

impl FleetChart {
    pub fn add_repo(&self) -> RepoAddResult<Child> {
        Ok(Command::new("helm")
            .args(["repo", "add", "fleet", &self.repo])
            .spawn()?)
    }

    pub fn update_repo(&self) -> RepoUpdateResult<Child> {
        Ok(Command::new("helm")
            .args(["repo", "update", "fleet"])
            .spawn()?)
    }

    pub fn search_repo(&self) -> RepoSearchResult<Vec<ChartSearch>> {
        let result = Command::new("helm")
            .stdout(Stdio::piped())
            .args(["search", "repo", "fleet", "-o", "json"])
            .spawn()?
            .wait_with_output()?;

        let output = &String::from_utf8(result.stdout)?;
        Ok(serde_json::from_str(output)?)
    }

    pub fn get_metadata(&self, chart: &str) -> MetadataGetResult<Option<ChartInfo>> {
        let mut metadata = Command::new("helm");
        metadata.args(["list", "-A", "-o", "json"]);

        let run = metadata
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let result = run.wait_with_output()?;
        let error = String::from_utf8(result.stderr)?;
        if result.status.code() == Some(1) && &error == "Error: release: not found" {
            return Ok(None);
        }

        let output = &String::from_utf8(result.stdout)?;
        let infos: Vec<ChartInfo> = serde_json::from_str(output)?;

        Ok(infos.into_iter().find(|i| i.name == chart))
    }

    pub fn fleet(&self, operation: &str) -> FleetInstallResult<Child> {
        let mut install = Command::new("helm");

        install.args([operation, "fleet", "fleet/fleet"]);

        if self.create_namespace {
            install.arg("--create-namespace");
        }

        if !self.namespace.is_empty() {
            install.args(["--namespace", &self.namespace]);
        }

        match self.version.clone() {
            Install::FollowLatest(_) => {}
            Install::Version(version) => {
                install.args(["--version", &version]);
            }
        }

        if self.wait {
            install.arg("--wait");
        }

        install.args([
            "--set",
            &format!("bootstrap.enabled={}", self.bootstrap_local_cluster),
            "--set-string",
            "extraEnv[0].name=EXPERIMENTAL_HELM_OPS",
            "--set-string",
            &format!("extraEnv[0].value={}", self.experimental_oci_ops),
        ]);

        Ok(install.spawn()?)
    }

    pub fn fleet_crds(&self, operation: &str) -> FleetCRDInstallResult<Child> {
        let mut install = Command::new("helm");

        install.args([operation, "fleet-crd", "fleet/fleet-crd"]);

        if self.create_namespace {
            install.arg("--create-namespace");
        }

        if !self.namespace.is_empty() {
            install.args(["--namespace", &self.namespace]);
        }

        match self.version.clone() {
            Install::FollowLatest(_) => {}
            Install::Version(version) => {
                install.args(["--version", &version]);
            }
        }

        if self.wait {
            install.arg("--wait");
        }

        Ok(install.spawn()?)
    }
}
