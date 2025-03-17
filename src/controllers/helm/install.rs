use std::{
    fmt::Display,
    process::{Child, Command, Stdio},
};

use serde::Deserialize;

use crate::api::fleet_addon_config::Install;

use super::{
    FleetCRDInstallResult, FleetInstallResult, FleetPatchResult, MetadataGetResult, RepoAddResult,
    RepoSearchResult, RepoUpdateResult,
};

#[derive(Default, Clone)]
pub struct FleetChart {
    pub repo: String,
    pub version: Option<Install>,
    pub namespace: String,

    pub wait: bool,
    pub update_dependency: bool,
    pub create_namespace: bool,

    pub bootstrap_local_cluster: bool,
}

#[derive(PartialEq)]
pub enum HelmOperation {
    Install,
    Upgrade,
}

impl Display for HelmOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HelmOperation::Install => f.write_str("install"),
            HelmOperation::Upgrade => f.write_str("upgrade"),
        }
    }
}

#[derive(Default, Clone)]
pub struct FleetOptions {
    pub namespace: String,
    pub experimental_oci_storage: bool,
    pub experimental_helm_ops: bool,
}

impl Display for FleetOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            namespace,
            experimental_oci_storage,
            experimental_helm_ops,
        } = self;
        f.write_str(&format!(
            "ns={namespace}, oci={experimental_oci_storage}, helm={experimental_helm_ops}"
        ))
    }
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

    pub fn get_metadata(chart: &str) -> MetadataGetResult<Option<ChartInfo>> {
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

    pub fn fleet(&self, operation: &HelmOperation) -> FleetInstallResult<Child> {
        let mut install = Command::new("helm");

        install.args([&operation.to_string(), "fleet", "fleet/fleet"]);

        if operation == &HelmOperation::Upgrade {
            install.arg("--reuse-values");
        }

        if self.create_namespace {
            install.arg("--create-namespace");
        }

        if !self.namespace.is_empty() {
            install.args(["--namespace", &self.namespace]);
        }

        match self.version.clone().unwrap_or_default() {
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
        ]);

        Ok(install.spawn()?)
    }

    pub fn fleet_crds(&self, operation: &HelmOperation) -> FleetCRDInstallResult<Child> {
        let mut install = Command::new("helm");

        install.args([&operation.to_string(), "fleet-crd", "fleet/fleet-crd"]);

        if operation == &HelmOperation::Upgrade {
            install.arg("--reuse-values");
        }

        if self.create_namespace {
            install.arg("--create-namespace");
        }

        if !self.namespace.is_empty() {
            install.args(["--namespace", &self.namespace]);
        }

        match self.version.clone().unwrap_or_default() {
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

impl FleetOptions {
    pub fn patch_fleet(&self) -> FleetPatchResult<Child> {
        let mut upgrade = Command::new("helm");

        upgrade.args(["upgrade", "fleet", "fleet/fleet", "--reuse-values"]);

        if !self.namespace.is_empty() {
            upgrade.args(["--namespace", &self.namespace]);
        }

        upgrade.arg("--wait");

        upgrade.args([
            "--set-string",
            "extraEnv[0].name=EXPERIMENTAL_OCI_STORAGE",
            "--set-string",
            &format!("extraEnv[0].value={}", self.experimental_oci_storage),
            "--set-string",
            "extraEnv[1].name=EXPERIMENTAL_HELM_OPS",
            "--set-string",
            &format!("extraEnv[1].value={}", self.experimental_helm_ops),
        ]);

        Ok(upgrade.spawn()?)
    }
}
