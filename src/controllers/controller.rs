use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::metrics::Diagnostics;
use crate::{telemetry, Error, Metrics};
use chrono::Utc;

use k8s_openapi::NamespaceResourceScope;

use kube::api::PostParams;

use kube::runtime::events::{Event, EventType};
use kube::runtime::finalizer;

use kube::{api::Api, client::Client, runtime::controller::Action};

use serde::de::DeserializeOwned;
use serde::Serialize;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{self, debug, info, instrument};

use super::{GetOrCreateError, SyncError};

pub static FLEET_FINALIZER: &str = "fleet.addons.cluster.x-k8s.io";

// Context for the reconciler
#[derive(Clone)]
pub struct Context {
    /// Kubernetes client
    pub client: Client,
    /// Diagnostoics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prom metrics
    pub metrics: Metrics,
}

pub(crate) async fn get_or_create<R>(ctx: Arc<Context>, res: R) -> Result<Action, GetOrCreateError>
where
    R: std::fmt::Debug,
    R: Clone + Serialize + DeserializeOwned,
    R: kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>,
    R: kube::ResourceExt,
{
    let ns = res
        .meta()
        .namespace
        .clone()
        .unwrap_or(String::from("default"));
    let api: Api<R> = Api::namespaced(ctx.client.clone(), &ns);

    let obj = api
        .get_metadata_opt(res.name_any().as_str())
        .await
        .map_err(GetOrCreateError::Lookup)?;

    if obj.is_some() {
        return Ok(Action::await_change());
    }

    api.create(&PostParams::default(), &res)
        .await
        .map_err(GetOrCreateError::Create)?;

    info!("Created fleet object");
    ctx.diagnostics
        .read()
        .await
        .recorder(ctx.client.clone(), &res)
        // Record object creation
        .publish(Event {
            type_: EventType::Normal,
            reason: "Created".into(),
            note: Some(format!(
                "Created fleet object `{}` in `{}`",
                res.name_any(),
                res.namespace().unwrap_or_default()
            )),
            action: "Creating".into(),
            secondary: None,
        })
        .await
        .map_err(GetOrCreateError::Event)?;

    Ok(Action::await_change())
}

pub(crate) trait FleetBundle {
    async fn sync(&self, ctx: Arc<Context>) -> crate::Result<Action>;
}

pub(crate) trait FleetController
where
    Self: std::fmt::Debug,
    Self: Clone + Serialize + DeserializeOwned,
    Self: kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>,
    Self: kube::ResourceExt,
{
    type Bundle: FleetBundle;

    #[instrument(skip_all, fields(trace_id = display(telemetry::get_trace_id()), name = self.name_any(), namespace = self.namespace()))]
    async fn reconcile(self: Arc<Self>, ctx: Arc<Context>) -> crate::Result<Action> {
        let name = self.name_any();
        let namespace = self.namespace().unwrap_or_default();
        ctx.diagnostics.write().await.last_event = Utc::now();

        let config_api: Api<FleetAddonConfig> = Api::all(ctx.client.clone());
        let config = config_api
            .get_opt("fleet-addon-config")
            .await
            .map_err(Error::ConfigFetch)?
            .unwrap_or_default();

        let cluster_api: Api<Self> = Api::namespaced(ctx.client.clone(), namespace.as_str());
        debug!("Reconciling \"{}\" in {}", name, namespace);

        finalizer(&cluster_api, FLEET_FINALIZER, self, |event| async {
            let r = match event {
                finalizer::Event::Apply(c) => c.to_bundle(&config)?.sync(ctx).await,
                finalizer::Event::Cleanup(c) => c.cleanup(ctx).await,
            };

            match r {
                Ok(r) => Ok(r),
                Err(Error::FleetError(SyncError::EarlyReturn)) => Ok(Action::await_change()),
                Err(e) => Err(e),
            }
        })
        .await
        .map_err(|e| Error::FinalizerError(Box::new(e)))
    }

    async fn cleanup(&self, ctx: Arc<Context>) -> crate::Result<Action> {
        ctx.diagnostics
            .read()
            .await
            .recorder(ctx.client.clone(), self)
            // Cleanup is perfomed by owner reference
            .publish(Event {
                type_: EventType::Normal,
                reason: "DeleteRequested".into(),
                note: Some(format!("Delete `{}`", self.name_any())),
                action: "Deleting".into(),
                secondary: None,
            })
            .await?;

        Ok(Action::await_change())
    }

    fn to_bundle(&self, config: &FleetAddonConfig) -> crate::Result<Self::Bundle>;
}
