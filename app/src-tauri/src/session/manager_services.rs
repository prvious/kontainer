//! Swarm services, one service's newest task error, and scale / restart / rollback, as
//! credential-free IPC views.
//!
//! Mirrors `app/src/types/services.ts`; the shared fixture `app/tests/fixtures/services-ipc.json`
//! keeps both sides in sync.

use kontainer_native_client::{
    IdempotencyKey, ServiceAction, ServiceDetail, ServiceId, ServiceItem, ServiceList, ServiceMode,
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
pub(crate) struct ServiceListView {
    items: Vec<ServiceView>,
    snapshot_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceView {
    id: String,
    name: String,
    image: String,
    image_reference: String,
    mode: ServiceMode,
    desired_replicas: u64,
    running_replicas: u64,
    ports: Vec<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    version: u64,
    stack: Option<String>,
    update_state: Option<String>,
}

/// What the replica tooltip reads.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServiceDetailView {
    latest_error: Option<String>,
}

impl From<ServiceItem> for ServiceView {
    fn from(item: ServiceItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            image: item.image,
            image_reference: item.image_reference,
            mode: item.mode,
            desired_replicas: item.desired_replicas,
            running_replicas: item.running_replicas,
            ports: item.ports,
            created_at: item.created_at.map(|at| timestamp!(at)),
            updated_at: item.updated_at.map(|at| timestamp!(at)),
            version: item.version,
            stack: item.stack,
            update_state: item.update_state,
        }
    }
}

impl From<ServiceList> for ServiceListView {
    fn from(list: ServiceList) -> Self {
        Self {
            items: list.items.into_iter().map(ServiceView::from).collect(),
            snapshot_at: timestamp!(list.snapshot_at),
        }
    }
}

impl From<ServiceDetail> for ServiceDetailView {
    fn from(detail: ServiceDetail) -> Self {
        Self {
            latest_error: detail.latest_error,
        }
    }
}

impl SessionManager {
    pub(crate) async fn services(
        &self,
        profile_id: Uuid,
        stack: Option<&str>,
    ) -> Result<ServiceListView, InventoryError> {
        let stack = stack.map(str::to_owned);
        let list = self
            .authenticated(
                profile_id,
                &stack,
                |controller, access, cancellation, stack| {
                    controller.list_services(access, stack.as_deref(), cancellation)
                },
            )
            .await?;
        Ok(list.into())
    }

    pub(crate) async fn service(
        &self,
        profile_id: Uuid,
        service_id: &str,
    ) -> Result<ServiceDetailView, InventoryError> {
        let service_id = ServiceId::parse(service_id)?;
        let detail = self
            .authenticated(
                profile_id,
                &service_id,
                |controller, access, cancellation, service_id| {
                    controller.get_service(access, service_id, cancellation)
                },
            )
            .await?;
        Ok(detail.into())
    }

    pub(crate) async fn service_action(
        &self,
        profile_id: Uuid,
        service_id: &str,
        action: ServiceAction,
        version: u64,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            ServiceId::parse(service_id)?,
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (service_id, key)| {
                    controller.service_action(
                        access,
                        service_id,
                        action,
                        version,
                        key,
                        cancellation,
                    )
                },
            )
            .await?;
        Ok(admitted.into())
    }
}
