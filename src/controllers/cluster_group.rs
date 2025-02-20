use crate::api::fleet_clustergroup::ClusterGroup;

use kube::api::PatchParams;
use kube::runtime::controller::Action;

use std::sync::Arc;

use super::controller::{patch, Context, FleetBundle, FleetController};
use super::{BundleResult, GroupSyncResult};

impl FleetBundle for ClusterGroup {
    // Applies finalizer on the existing ClusterGroup object, so the deletion event is not missed
    #[allow(refining_impl_trait)]
    async fn sync(&self, ctx: Arc<Context>) -> GroupSyncResult<Action> {
        patch(
            ctx.clone(),
            self.clone(),
            &PatchParams::apply("addon-provider-fleet"),
        )
        .await?;

        Ok(Action::await_change())
    }
}

impl FleetController for ClusterGroup {
    type Bundle = ClusterGroup;

    async fn to_bundle(&self, _ctx: Arc<Context>) -> BundleResult<Option<Self::Bundle>> {
        Ok(Some(self.clone()))
    }
}
