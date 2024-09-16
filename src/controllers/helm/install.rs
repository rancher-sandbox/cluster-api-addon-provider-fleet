use std::process::{Child, Command, Stdio};

use serde::Deserialize;

use super::{FleetCRDInstallResult, FleetInstallResult, MetadataGetResult, RepoAddResult};

#[derive(Default)]
pub struct FleetChart {
    pub repo: String,
    pub version: String,
    pub namespace: String,

    pub wait: bool,
    pub update_dependency: bool,
    pub create_namespace: bool,
    pub bootstrap_local_cluster: bool,
}

#[derive(Deserialize)]
pub struct ChartInfo {
    pub name: String,
    pub namespace: String,
    pub app_version: String,
    pub status: String,
}

impl FleetChart {
    pub fn add_repo(&self) -> RepoAddResult<Child> {
        Ok(Command::new("helm")
            .args(["repo", "add", "fleet", &self.repo])
            .spawn()?)
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

        if self.namespace.len() > 0 {
            install.args(["--namespace", &self.namespace]);
        }

        if self.version.len() > 0 {
            install.args(["--version", &self.version]);
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

    pub fn fleet_crds(&self, operation: &str) -> FleetCRDInstallResult<Child> {
        let mut install = Command::new("helm");

        install.args([operation, "fleet-crd", "fleet/fleet-crd"]);

        if self.create_namespace {
            install.arg("--create-namespace");
        }

        if self.namespace.len() > 0 {
            install.args(["--namespace", &self.namespace]);
        }

        install.args(["--version", &self.version]);

        if self.wait {
            install.arg("--wait");
        }

        Ok(install.spawn()?)
    }
}
