use std::process::{Child, Command};

use super::{FleetCRDInstallResult, FleetInstallResult, RepoAddResult};

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

impl FleetChart {
    pub fn add_repo(&self) -> RepoAddResult<Child> {
        Ok(Command::new("helm")
            .args(["repo", "add", "fleet", &self.repo])
            .spawn()?)
    }

    pub fn install_fleet(&self) -> FleetInstallResult<Child> {
        let mut install = Command::new("helm");

        install.args(["install", "fleet", "fleet/fleet"]);

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

    pub fn install_fleet_crds(&self) -> FleetCRDInstallResult<Child> {
        let mut install = Command::new("helm");

        install.args(["install", "fleet-crd", "fleet/fleet-crd"]);

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
