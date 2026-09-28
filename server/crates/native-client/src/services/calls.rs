//! `GET /api/v1/services[/{serviceId}]` and `POST /api/v1/services/{serviceId}/actions/{action}`.

use reqwest::Method;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::{
    MAX_REPLICAS, ServiceAction, ServiceDetail, ServiceDetailBody, ServiceId, ServiceList,
    ServiceListBody,
};
use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::{AdmittedOperation, IdempotencyKey};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionBody {
    version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<u64>,
}

impl NativeClient {
    /// Every service (or one stack's) with running counts.
    pub async fn list_services(
        &self,
        access: &AccessCredential,
        stack: Option<&str>,
        cancellation: &CancellationToken,
    ) -> Result<ServiceList, NativeClientError> {
        let mut url = self.endpoint("/api/v1/services")?;
        if let Some(stack) = stack {
            if stack.is_empty() || stack.len() > 128 {
                return Err(NativeClientError::InvalidRequest);
            }
            url.query_pairs_mut().append_pair("stack", stack);
        }
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute_with_error_policy(
                request,
                RetryPolicy::Once,
                self.limits.collection_body_limit,
                cancellation,
                ErrorBodyPolicy::Inventory,
            )
            .await?;
        decode_json::<ServiceListBody>(&body)?.into_list()
    }

    /// One service with its newest tasks and the newest task error.
    pub async fn get_service(
        &self,
        access: &AccessCredential,
        service_id: &ServiceId,
        cancellation: &CancellationToken,
    ) -> Result<ServiceDetail, NativeClientError> {
        let url = self.endpoint(&format!("/api/v1/services/{}", service_id.as_str()))?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute_with_error_policy(
                request,
                RetryPolicy::Once,
                self.limits.collection_body_limit,
                cancellation,
                ErrorBodyPolicy::Inventory,
            )
            .await?;
        decode_json::<ServiceDetailBody>(&body)?.into_detail()
    }

    /// Admit one scale, restart, or rollback based on `version`.
    pub async fn service_action(
        &self,
        access: &AccessCredential,
        service_id: &ServiceId,
        action: ServiceAction,
        version: u64,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let (name, replicas) = match action {
            ServiceAction::Scale { replicas } if replicas <= MAX_REPLICAS => {
                ("scale", Some(replicas))
            }
            ServiceAction::Scale { .. } => return Err(NativeClientError::InvalidRequest),
            ServiceAction::Restart => ("restart", None),
            ServiceAction::Rollback => ("rollback", None),
        };
        let path = format!("/api/v1/services/{}/actions/{name}", service_id.as_str());
        self.admit(
            access,
            &path,
            key,
            &ActionBody { version, replicas },
            cancellation,
        )
        .await
    }
}
