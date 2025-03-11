use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::controllers::PatchError;
use crate::metrics::Diagnostics;
use crate::multi_dispatcher::{BroadcastStream, MultiDispatcher};
use crate::{telemetry, Error, Metrics};
use chrono::Utc;

use futures::stream::SelectAll;
use futures::Stream;
use k8s_openapi::NamespaceResourceScope;

use kube::api::{DynamicObject, Patch, PatchParams, PostParams};

use kube::runtime::events::{Event, EventType};
use kube::runtime::{finalizer, watcher};

use kube::{api::Api, client::Client, runtime::controller::Action};

use serde::de::DeserializeOwned;
use serde::Serialize;

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{self, debug, info, instrument};

use super::{
    BundleResult, ConfigFetchResult, GetOrCreateError, GetOrCreateResult, PatchResult, SyncError,
};

pub static FLEET_FINALIZER: &str = "fleet.addons.cluster.x-k8s.io";

type DynamicStream = SelectAll<
    Pin<Box<dyn Stream<Item = Result<watcher::Event<DynamicObject>, watcher::Error>> + Send>>,
>;

// Context for the reconciler
#[derive(Clone)]
pub struct Context {
    /// Kubernetes client
    pub client: Client,
    /// Diagnostoics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prom metrics
    pub metrics: Metrics,
    // Dispatcher for dynamic resource controllers
    pub dispatcher: MultiDispatcher,
    // shared stream of dynamic events
    pub stream: BroadcastStream<DynamicStream>,
}

pub(crate) async fn get_or_create<R>(ctx: Arc<Context>, res: R) -> GetOrCreateResult<Action>
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
    let api = Api::namespaced(ctx.client.clone(), &ns);

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
        .recorder(ctx.client.clone())
        // Record object creation
        .publish(
            &Event {
                type_: EventType::Normal,
                reason: "Created".into(),
                note: Some(format!(
                    "Created fleet object `{}` in `{}`",
                    res.name_any(),
                    res.namespace().unwrap_or_default()
                )),
                action: "Creating".into(),
                secondary: None,
            },
            &res.object_ref(&()),
        )
        .await?;

    Ok(Action::await_change())
}

pub(crate) async fn patch<R>(ctx: Arc<Context>, res: R, pp: &PatchParams) -> PatchResult<Action>
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

    let mut res = res.clone();
    res.meta_mut().managed_fields = None;

    api.patch(&res.name_any(), pp, &Patch::Apply(res.clone()))
        .await
        .map_err(PatchError::Patch)?;

    info!("Updated fleet object");
    ctx.diagnostics
        .read()
        .await
        .recorder(ctx.client.clone())
        // Record object creation
        .publish(
            &Event {
                type_: EventType::Normal,
                reason: "Updated".into(),
                note: Some(format!(
                    "Updated fleet object `{}` in `{}`",
                    res.name_any(),
                    res.namespace().unwrap_or_default()
                )),
                action: "Creating".into(),
                secondary: None,
            },
            &res.object_ref(&()),
        )
        .await?;

    Ok(Action::await_change())
}

pub(crate) async fn fetch_config(client: Client) -> ConfigFetchResult<FleetAddonConfig> {
    Ok(Api::all(client)
        .get_opt("fleet-addon-config")
        .await?
        .unwrap_or_default())
}

pub(crate) trait FleetBundle {
    async fn sync(&self, ctx: Arc<Context>) -> Result<Action, impl Into<SyncError>>;
}

pub(crate) trait FleetController
where
    Self: std::fmt::Debug,
    Self: Clone + Serialize + DeserializeOwned,
    Self: kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>,
    Self: kube::ResourceExt,
{
    type Bundle: FleetBundle;

    #[instrument(skip_all, fields(trace_id = display(telemetry::get_trace_id()), name = self.name_any(), namespace = self.namespace()), err)]
    async fn reconcile(self: Arc<Self>, ctx: Arc<Context>) -> crate::Result<Action> {
        ctx.diagnostics.write().await.last_event = Utc::now();

        let namespace = self.namespace().unwrap_or_default();
        let api = Api::namespaced(ctx.client.clone(), namespace.as_str());
        debug!("Reconciling");

        finalizer(&api, FLEET_FINALIZER, self, |event| async {
            match event {
                finalizer::Event::Apply(c) => match c.to_bundle(ctx.clone()).await? {
                    Some(bundle) => bundle
                        .sync(ctx)
                        .await
                        .map_err(Into::into)
                        .map_err(Into::into),
                    _ => Ok(Action::await_change()),
                },
                finalizer::Event::Cleanup(c) => c.cleanup(ctx).await,
            }
        })
        .await
        .map_err(|e| Error::FinalizerError(Box::new(e)))
    }

    async fn cleanup(&self, ctx: Arc<Context>) -> crate::Result<Action> {
        ctx.diagnostics
            .read()
            .await
            .recorder(ctx.client.clone())
            // Cleanup is perfomed by owner reference
            .publish(
                &Event {
                    type_: EventType::Normal,
                    reason: "DeleteRequested".into(),
                    note: Some(format!("Delete `{}`", self.name_any())),
                    action: "Deleting".into(),
                    secondary: None,
                },
                &self.object_ref(&()),
            )
            .await?;

        Ok(Action::await_change())
    }

    async fn to_bundle(&self, ctx: Arc<Context>) -> BundleResult<Option<Self::Bundle>>;
}
