//! `POST /api/v1/containers/{nodeId}/{containerId}/actions/{action}` and
//! `POST /api/v1/containers/actions/bulk`.

use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::{
    ContainerAction, ContainerActionOptions, ContainerTarget, MAX_BULK_TARGETS,
    MAX_STOP_TIMEOUT_SECONDS, valid_container_id,
};
use crate::client::NativeClient;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::{AdmittedOperation, IdempotencyKey};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionBody {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    force: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContainerTargetBody<'a> {
    node_id: &'a str,
    container_id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkBody<'a> {
    action: &'a str,
    targets: Vec<ContainerTargetBody<'a>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    force: bool,
}

impl NativeClient {
    /// Admit one lifecycle action. Invalid ids or a timeout above 600 s fail locally.
    pub async fn container_action(
        &self,
        access: &AccessCredential,
        target: &ContainerTarget,
        action: ContainerAction,
        options: ContainerActionOptions,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let container_id = target.container_id.as_str();
        if !valid_container_id(container_id)
            || options
                .timeout_seconds
                .is_some_and(|seconds| seconds > MAX_STOP_TIMEOUT_SECONDS)
        {
            return Err(NativeClientError::InvalidRequest);
        }
        let path = format!(
            "/api/v1/containers/{}/{container_id}/actions/{}",
            target.node_id.as_str(),
            action.as_str()
        );
        let body = ActionBody {
            force: options.force,
            timeout_seconds: options.timeout_seconds,
        };
        self.admit(access, &path, key, &body, cancellation).await
    }

    /// Admit one action across 1–200 distinct containers as a single `containers.bulk`
    /// operation.
    pub async fn containers_bulk_action(
        &self,
        access: &AccessCredential,
        action: ContainerAction,
        targets: &[ContainerTarget],
        force: bool,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let distinct: std::collections::HashSet<_> = targets.iter().collect();
        if targets.is_empty()
            || targets.len() > MAX_BULK_TARGETS
            || distinct.len() != targets.len()
            || !targets
                .iter()
                .all(|target| valid_container_id(&target.container_id))
        {
            return Err(NativeClientError::InvalidRequest);
        }
        let body = BulkBody {
            action: action.as_str(),
            targets: targets
                .iter()
                .map(|target| ContainerTargetBody {
                    node_id: target.node_id.as_str(),
                    container_id: &target.container_id,
                })
                .collect(),
            force,
        };
        self.admit(
            access,
            "/api/v1/containers/actions/bulk",
            key,
            &body,
            cancellation,
        )
        .await
    }
}
