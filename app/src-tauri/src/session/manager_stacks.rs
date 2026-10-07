//! Stacks, their removal plan, and removal, as credential-free IPC views.
//!
//! Mirrors `app/src/types/stacks.ts`; the shared fixture `app/tests/fixtures/stacks-ipc.json`
//! keeps both sides in sync.

use kontainer_native_client::{
    IdempotencyKey, StackItem, StackList, StackName, StackRemovalPlan, StackStatus,
};
use serde::Serialize;
use uuid::Uuid;

use super::{inventory::InventoryError, operations::AdmittedView, SessionManager};

/// The controller's canonical RFC 3339 millisecond form.
macro_rules! timestamp {
    ($value:expr) => {
        $value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    };
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackListView {
    items: Vec<StackView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackView {
    name: String,
    status: StackStatus,
    services: u32,
    running_containers: u64,
    desired_containers: u64,
    cpu_percent: Option<f64>,
    memory_bytes: Option<u64>,
    last_deployed: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackPlanView {
    services: Vec<String>,
    networks: Vec<String>,
    volumes_retained: Vec<String>,
}

impl From<StackItem> for StackView {
    fn from(item: StackItem) -> Self {
        Self {
            name: item.name,
            status: item.status,
            services: item.services,
            running_containers: item.running_containers,
            desired_containers: item.desired_containers,
            cpu_percent: item.cpu_percent,
            memory_bytes: item.memory_bytes,
            last_deployed: item.last_deployed.map(|at| timestamp!(at)),
        }
    }
}

impl From<StackList> for StackListView {
    fn from(list: StackList) -> Self {
        Self {
            items: list.items.into_iter().map(StackView::from).collect(),
        }
    }
}

impl From<StackRemovalPlan> for StackPlanView {
    fn from(plan: StackRemovalPlan) -> Self {
        Self {
            services: plan.services,
            networks: plan.networks,
            volumes_retained: plan.volumes_retained,
        }
    }
}

impl SessionManager {
    pub(crate) async fn stacks(&self, profile_id: Uuid) -> Result<StackListView, InventoryError> {
        let list = self
            .authenticated(profile_id, &(), |controller, access, cancellation, ()| {
                controller.list_stacks(access, cancellation)
            })
            .await?;
        Ok(list.into())
    }

    pub(crate) async fn stack_plan(
        &self,
        profile_id: Uuid,
        name: &str,
    ) -> Result<StackPlanView, InventoryError> {
        let name = StackName::parse(name)?;
        let plan = self
            .authenticated(
                profile_id,
                &name,
                |controller, access, cancellation, name| {
                    controller.stack_removal_plan(access, name, cancellation)
                },
            )
            .await?;
        Ok(plan.into())
    }

    pub(crate) async fn remove_stack(
        &self,
        profile_id: Uuid,
        name: &str,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            StackName::parse(name)?,
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (name, key)| {
                    controller.remove_stack(access, name, key, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }
}
