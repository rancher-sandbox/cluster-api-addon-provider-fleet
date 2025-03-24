use crate::api::fleet_clustergroup::ClusterGroup;

use cluster_api_rs::capi_clusterclass::ClusterClass;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde_json::json;

use std::ops::Deref;
use std::sync::Arc;

use super::controller::{patch, Context, FLEET_FINALIZER};
use super::{GroupSyncResult, SyncError};

impl ClusterGroup {
    pub async fn reconcile(self: Arc<Self>, ctx: Arc<Context>) -> crate::Result<Action> {
        let mut group = self.deref().clone();
        Ok(group.sync(ctx).await.map_err(SyncError::from)?)
    }

    async fn sync(&mut self, ctx: Arc<Context>) -> GroupSyncResult<Action> {
        if let Some(cc_ref) = self.cluster_class_ref() {
            let class = ctx.client.fetch::<ClusterClass>(&cc_ref).await?;
            self.labels_mut().extend(
                class
                    .labels()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string())),
            );

            patch(
                ctx.clone(),
                self,
                &PatchParams::apply("addon-provider-fleet"),
            )
            .await?;
        }

        if self.finalizers().iter().any(|f| f == FLEET_FINALIZER) {
            self.finalizers_mut().retain(|f| f != FLEET_FINALIZER);
            let api: Api<Self> =
                Api::namespaced(ctx.client.clone(), &self.namespace().unwrap_or_default());
            api.patch(
                &self.name_any(),
                &Default::default(),
                &Patch::Merge(json!({"metadata": {"finalizers": self.finalizers()}})),
            )
            .await?;
        }

        Ok(Action::await_change())
    }
}
