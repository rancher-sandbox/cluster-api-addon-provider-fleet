use base64::prelude::*;
use std::{fmt::Display, str::FromStr, sync::Arc};

use k8s_openapi::api::core::v1::{ConfigMap, Endpoints};
use kube::{
    api::{ObjectMeta, Patch, PatchParams, TypeMeta},
    client::scope::Namespace,
    core::object::HasSpec,
    runtime::controller::Action,
    Api, Resource, ResourceExt,
};
use serde::{ser, Deserialize, Serialize};
use serde_json::Value;
use serde_with::{serde_as, DisplayFromStr};
use thiserror::Error;
use tracing::instrument;

use crate::{
    api::fleet_addon_config::{FleetAddonConfig, InstallOptions, Server},
    telemetry,
};

use super::controller::Context;

#[derive(Resource, Serialize, Deserialize, Default, Clone, Debug)]
#[resource(inherit = ConfigMap)]
struct FleetConfig {
    #[serde(flatten, default)]
    types: Option<TypeMeta>,
    metadata: ObjectMeta,
    data: FleetConfigSpec,
}

#[serde_as]
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
struct FleetConfigSpec {
    #[serde_as(as = "DisplayFromStr")]
    config: FleetConfigData,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
struct FleetConfigData {
    #[serde(rename = "apiServerURL")]
    api_server_url: String,

    #[serde(rename = "apiServerCA")]
    api_server_ca: String,

    #[serde(flatten)]
    other: Value,
}

impl FromStr for FleetConfigData {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl Display for FleetConfigData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).map_err(ser::Error::custom)?)
    }
}

#[derive(Resource, Deserialize, Serialize, Clone, Debug)]
#[resource(inherit = ConfigMap)]
struct CertConfigMap {
    metadata: ObjectMeta,
    data: CertData,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct CertData {
    #[serde(rename = "ca.crt")]
    ca_crt: String,
}

impl FleetAddonConfig {
    #[instrument(skip_all, fields(trace_id = display(telemetry::get_trace_id()), name = self.name_any(), namespace = self.namespace()))]
    pub async fn reconcile(self: Arc<Self>, ctx: Arc<Context>) -> crate::Result<Action> {
        self.reconcile_config_sync(ctx).await.map_err(Into::into)
    }

    #[instrument(skip_all, fields(trace_id = display(telemetry::get_trace_id()), name = self.name_any(), namespace = self.namespace()))]
    pub async fn reconcile_config_sync(
        self: Arc<Self>,
        ctx: Arc<Context>,
    ) -> AddonConfigSyncResult<Action> {
        if self.name_any() != "fleet-addon-config" {
            return Ok(Action::await_change());
        }

        let ns = Namespace::from("cattle-fleet-system");
        let mut fleet_config: FleetConfig = ctx.client.get("fleet-controller", &ns).await?;

        if let Some(config) = self.spec().config.as_ref() {
            self.update_certificate(ctx.clone(), &mut fleet_config, &config.server)
                .await?;
            self.update_url(ctx.clone(), &mut fleet_config, &config.server)
                .await?;
        }

        fleet_config.metadata.managed_fields = None;
        fleet_config.types = Some(TypeMeta::resource::<FleetConfig>());

        let api: Api<FleetConfig> = Api::namespaced(ctx.client.clone(), "cattle-fleet-system");
        api.patch(
            &fleet_config.name_any(),
            &PatchParams::apply("addon-provider-fleet").force(),
            &Patch::Apply(&fleet_config),
        )
        .await?;

        Ok(Action::await_change())
    }

    fn default_endpoint_lookup(&self, endpoints: Endpoints) -> Option<String> {
        let subsets = endpoints.subsets?;
        let subnet = subsets.first()?.clone();
        let addresses = subnet.addresses?;
        let ports = subnet.ports?;
        let address = addresses.first()?.clone();
        let port = ports.first()?.clone();

        let url = address.hostname.or(Some(address.ip))?;
        let name = port.name;
        let port = port.port;
        match name {
            Some(name) => Some(format!("{name}://{url}:{port}")),
            None => Some(url),
        }
    }

    async fn update_certificate(
        &self,
        ctx: Arc<Context>,
        fleet_config: &mut FleetConfig,
        fleet_install: &Server,
    ) -> AddonConfigSyncResult<()> {
        let ns = Namespace::from("default");
        let cert_config_map: CertConfigMap = match fleet_install {
            Server::InferLocal(true) => ctx.client.get("kube-root-ca.crt", &ns).await?,
            Server::Custom(InstallOptions {
                api_server_ca_config_ref: Some(config_ref),
                ..
            }) => ctx.client.fetch(config_ref).await?,
            _ => return Ok(()),
        };

        fleet_config.data.config.api_server_ca =
            BASE64_STANDARD.encode(cert_config_map.data.ca_crt);

        Ok(())
    }

    async fn update_url(
        &self,
        ctx: Arc<Context>,
        fleet_config: &mut FleetConfig,
        fleet_install: &Server,
    ) -> AddonConfigSyncResult<()> {
        let api_server_url = match fleet_install {
            Server::InferLocal(true) => {
                if let Some(api_server_url) = {
                    let ns = Namespace::from("default");
                    self.default_endpoint_lookup(ctx.client.get("kubernetes", &ns).await?)
                } {
                    api_server_url
                } else {
                    return Ok(());
                }
            }
            Server::Custom(InstallOptions {
                api_server_url: Some(api_server_url),
                ..
            }) => api_server_url.clone(),
            _ => return Ok(()),
        };

        fleet_config.data.config.api_server_url = api_server_url;

        Ok(())
    }
}

pub type AddonConfigSyncResult<T> = std::result::Result<T, AddonConfigSyncError>;

#[derive(Error, Debug)]
pub enum AddonConfigSyncError {
    #[error("Certificate config map fetch error: {0}")]
    CertificateConfigMapFetch(#[from] kube::Error),
}

mod tests {
    #[test]
    fn test() {
        use crate::controllers::addon_config::FleetConfigData;
        let data = r#"{
            "systemDefaultRegistry": "",
            "agentImage": "rancher/fleet-agent:v0.9.4",
            "agentImagePullPolicy": "IfNotPresent",
            "apiServerURL": "https://192.168.1.123:43473",
            "apiServerCA": "test",
            "agentCheckinInterval": "15m",
            "ignoreClusterRegistrationLabels": false,
            "bootstrap": {
              "paths": "",
              "repo": "",
              "secret": "",
              "branch":  "master",
              "namespace": "fleet-local",
              "agentNamespace": ""
            },
            "webhookReceiverURL": "",
            "githubURLPrefix": ""
          }"#;

        let _config: FleetConfigData = serde_json::from_str(data).unwrap();
    }
}
