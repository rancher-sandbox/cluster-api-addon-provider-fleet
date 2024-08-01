use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::Result;

use kube::runtime::controller::Action;

use std::sync::Arc;

use super::controller::{patch, Context, FleetBundle, FleetController};
use super::{GroupSyncError, SyncError};

impl FleetBundle for ClusterGroup {
    // Applies finalizer on the existing ClusterGroup object, so the deletion event is not missed
    async fn sync(&self, ctx: Arc<Context>) -> Result<Action> {
        patch(ctx.clone(), self.clone())
            .await
            .map_err(Into::<GroupSyncError>::into)
            .map_err(Into::<SyncError>::into)?;

        Ok(Action::await_change())
    }
}

impl FleetController for ClusterGroup {
    type Bundle = ClusterGroup;

    async fn to_bundle(
        &self,
        _ctx: Arc<Context>,
        _config: &FleetAddonConfig,
    ) -> Result<Self::Bundle> {
        Ok(self.clone())
    }
}
