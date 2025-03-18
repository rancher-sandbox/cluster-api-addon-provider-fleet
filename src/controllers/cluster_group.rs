use crate::api::bundle_namespace_mapping::BundleNamespaceMapping;
use crate::api::fleet_clustergroup::ClusterGroup;

use cluster_api_rs::capi_clusterclass::ClusterClass;
use kube::api::PatchParams;
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};

use std::sync::Arc;

use super::controller::{patch, Context, FleetBundle, FleetController};
use super::{BundleResult, GroupSyncResult};

impl FleetBundle for ClusterGroup {
    // Applies finalizer on the existing ClusterGroup object, so the deletion event is not missed
    #[allow(refining_impl_trait)]
    async fn sync(&self, ctx: Arc<Context>) -> GroupSyncResult<Action> {
        if let Some(cc_ref) = self.cluster_class_ref() {
            let class = ctx.client.fetch::<ClusterClass>(&cc_ref).await?;
            patch(
                ctx.clone(),
                &{
                    let mut group = self.clone();
                    let labels = group.labels_mut();
                    class.labels().iter().for_each(|(key, value)| {
                        labels.insert(key.to_string(), value.to_string());
                    });
                    group
                },
                &PatchParams::apply("addon-provider-fleet"),
            )
            .await?;
        }

        Ok(Action::await_change())
    }

    async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action, super::SyncError> {
        let class_ns = self.cluster_class_namespace();
        let namespace = self.namespace();
        if class_ns.is_some() && class_ns != namespace {
            let api = Api::<BundleNamespaceMapping>::namespaced(
                ctx.client.clone(),
                &class_ns.unwrap_or_default(),
            );
            api.delete(&namespace.unwrap_or_default(), &Default::default())
                .await?;
        }
        Ok(Action::await_change())
    }
}

impl FleetController for ClusterGroup {
    type Bundle = ClusterGroup;

    async fn to_bundle(&self, _ctx: Arc<Context>) -> BundleResult<Option<Self::Bundle>> {
        Ok(Some(self.clone()))
    }
}
