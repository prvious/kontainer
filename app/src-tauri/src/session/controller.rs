//! Minimal controller port over the UI-neutral native client.
//!
//! The port exists so the custody state machine can be tested with scripted
//! outcomes and so every transport failure maps to exactly one closed session
//! code at the boundary. It adds no endpoint of its own.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use kontainer_native_client::{
    AccessCredential, AccountFieldErrors, AdmittedOperation, ContainerAction,
    ContainerActionOptions, ContainerList, ContainerTarget, ControllerSettings, IdempotencyKey,
    ImageList, ImageNodes, NativeClient, NativeClientConfig, NativeClientError, NodeId, NodeList,
    OperationEvent, OperationSnapshot, PasswordSession, ProbeFailure, RefreshCredential,
    SampleSeries, Traced, VolumeList, VolumeName, VolumeQuery, VolumeSpec,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::errors::{FieldErrorsView, LoginFailure, SessionError};

/// Probes answer within two seconds so the connect card never hangs.
const PROBE_DEADLINE: Duration = Duration::from_secs(2);

/// Boxed future used by the object-safe controller trait.
pub(crate) type ControllerFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The credential pair issued by sign-in, registration, or rotation.
pub(crate) struct IssuedCredentials {
    pub access: AccessCredential,
    pub refresh: RefreshCredential,
    pub access_expires_at_unix_ms: i64,
    pub refresh_expires_at_unix_ms: i64,
}

/// The authenticated identity plus its freshly issued credentials.
pub(crate) struct RedeemedSession {
    pub cluster_id: String,
    pub principal_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub issued: IssuedCredentials,
}

/// The one-per-origin controller surface custody depends on.
pub(crate) trait Controller: Send + Sync + 'static {
    /// The validated origin this controller is bound to.
    fn canonical_origin(&self) -> &str;

    fn bootstrap_status<'a>(
        &'a self,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<bool, LoginFailure>>;

    fn password_login<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<RedeemedSession, LoginFailure>>;

    fn register_owner<'a>(
        &'a self,
        display_name: &'a str,
        email: &'a str,
        password: &'a str,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<RedeemedSession, LoginFailure>>;

    /// `/health/ready` within two seconds.
    fn probe<'a>(
        &'a self,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<(), ProbeFailure>>;

    fn refresh_session<'a>(
        &'a self,
        refresh: &'a RefreshCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<IssuedCredentials, SessionError>>;

    fn logout<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<(), SessionError>>;

    fn list_nodes<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<NodeList, NativeClientError>>;

    /// `node_id` `None` reads the cluster series.
    fn samples<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: Option<&'a NodeId>,
        window: &'a str,
        bucket: &'a str,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<SampleSeries, NativeClientError>>;

    fn get_settings<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ControllerSettings, NativeClientError>>;

    fn update_settings<'a>(
        &'a self,
        access: &'a AccessCredential,
        metrics_retention_days: u32,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ControllerSettings, NativeClientError>>;

    fn list_containers<'a>(
        &'a self,
        access: &'a AccessCredential,
        node: Option<&'a NodeId>,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ContainerList, NativeClientError>>;

    fn container_action<'a>(
        &'a self,
        access: &'a AccessCredential,
        target: &'a ContainerTarget,
        action: ContainerAction,
        options: ContainerActionOptions,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>>;

    fn containers_bulk_action<'a>(
        &'a self,
        access: &'a AccessCredential,
        action: ContainerAction,
        targets: &'a [ContainerTarget],
        force: bool,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>>;

    fn list_images<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ImageList, NativeClientError>>;

    fn remove_image<'a>(
        &'a self,
        access: &'a AccessCredential,
        image_id: &'a str,
        nodes: &'a ImageNodes,
        force: bool,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>>;

    fn pull_image<'a>(
        &'a self,
        access: &'a AccessCredential,
        reference: &'a str,
        nodes: &'a [NodeId],
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>>;

    fn list_volumes<'a>(
        &'a self,
        access: &'a AccessCredential,
        query: &'a VolumeQuery,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<VolumeList, NativeClientError>>;

    fn create_volume<'a>(
        &'a self,
        access: &'a AccessCredential,
        spec: &'a VolumeSpec,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>>;

    fn remove_volume<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: &'a NodeId,
        name: &'a VolumeName,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>>;

    fn get_operation<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<OperationSnapshot, NativeClientError>>;

    fn list_operation_events<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        after: u32,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Vec<OperationEvent>, NativeClientError>>;

    fn cancel_operation<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<OperationSnapshot, NativeClientError>>;

    /// POST one mutation with its idempotency key.
    fn admit<'a>(
        &'a self,
        access: &'a AccessCredential,
        path: &'a str,
        key: &'a IdempotencyKey,
        body: &'a serde_json::Value,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>>;
}

/// Builds a controller for a validated controller origin.
pub(crate) trait ControllerFactory: Send + Sync + 'static {
    fn for_origin(&self, origin: &str) -> Result<Arc<dyn Controller>, SessionError>;
}

/// Production factory: validates the origin, then builds the native clients.
pub(crate) struct NativeControllerFactory;

impl ControllerFactory for NativeControllerFactory {
    fn for_origin(&self, origin: &str) -> Result<Arc<dyn Controller>, SessionError> {
        let client = NativeClient::new(NativeClientConfig::new(origin))
            .map_err(|_| SessionError::InvalidOrigin)?;
        let mut probe_config = NativeClientConfig::new(origin);
        probe_config.connect_timeout = PROBE_DEADLINE;
        probe_config.request_deadline = PROBE_DEADLINE;
        let probe_client =
            NativeClient::new(probe_config).map_err(|_| SessionError::InvalidOrigin)?;
        Ok(Arc::new(NativeController {
            client,
            probe_client,
        }))
    }
}

struct NativeController {
    client: NativeClient,
    probe_client: NativeClient,
}

fn redeemed_from(session: PasswordSession) -> RedeemedSession {
    RedeemedSession {
        cluster_id: session.cluster_id,
        principal_id: session.principal_id,
        email: session.email,
        display_name: session.display_name,
        issued: IssuedCredentials {
            access: session.credentials.access,
            refresh: session.credentials.refresh,
            access_expires_at_unix_ms: session.credentials.access_expires_at.timestamp_millis(),
            refresh_expires_at_unix_ms: session.credentials.refresh_expires_at.timestamp_millis(),
        },
    }
}

impl Controller for NativeController {
    fn canonical_origin(&self) -> &str {
        self.client.controller_origin()
    }

    fn bootstrap_status<'a>(
        &'a self,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<bool, LoginFailure>> {
        Box::pin(async move {
            self.client
                .bootstrap_status(cancellation)
                .await
                .map_err(map_account_failure)
        })
    }

    fn password_login<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<RedeemedSession, LoginFailure>> {
        Box::pin(async move {
            self.client
                .password_login(email, password, cancellation)
                .await
                .map(redeemed_from)
                .map_err(map_account_failure)
        })
    }

    fn register_owner<'a>(
        &'a self,
        display_name: &'a str,
        email: &'a str,
        password: &'a str,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<RedeemedSession, LoginFailure>> {
        Box::pin(async move {
            self.client
                .register_owner(display_name, email, password, cancellation)
                .await
                .map(redeemed_from)
                .map_err(map_account_failure)
        })
    }

    fn probe<'a>(
        &'a self,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<(), ProbeFailure>> {
        Box::pin(self.probe_client.probe_ready(cancellation))
    }

    fn refresh_session<'a>(
        &'a self,
        refresh: &'a RefreshCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<IssuedCredentials, SessionError>> {
        Box::pin(async move {
            self.client
                .refresh_session(refresh, cancellation)
                .await
                .map(|credentials| IssuedCredentials {
                    access: credentials.access,
                    refresh: credentials.refresh,
                    access_expires_at_unix_ms: credentials.access_expires_at.timestamp_millis(),
                    refresh_expires_at_unix_ms: credentials.refresh_expires_at.timestamp_millis(),
                })
                .map_err(map_refresh_error)
        })
    }

    fn logout<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<(), SessionError>> {
        Box::pin(async move {
            self.client
                .logout(access, cancellation)
                .await
                .map_err(map_logout_error)
        })
    }

    fn list_nodes<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<NodeList, NativeClientError>> {
        Box::pin(self.client.list_nodes(access, cancellation))
    }

    fn samples<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: Option<&'a NodeId>,
        window: &'a str,
        bucket: &'a str,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<SampleSeries, NativeClientError>> {
        Box::pin(async move {
            match node_id {
                Some(node_id) => {
                    self.client
                        .node_samples(access, node_id, window, bucket, cancellation)
                        .await
                }
                None => {
                    self.client
                        .cluster_samples(access, window, bucket, cancellation)
                        .await
                }
            }
        })
    }

    fn get_settings<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ControllerSettings, NativeClientError>> {
        Box::pin(self.client.get_settings(access, cancellation))
    }

    fn update_settings<'a>(
        &'a self,
        access: &'a AccessCredential,
        metrics_retention_days: u32,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ControllerSettings, NativeClientError>> {
        Box::pin(
            self.client
                .update_settings(access, metrics_retention_days, cancellation),
        )
    }

    fn list_containers<'a>(
        &'a self,
        access: &'a AccessCredential,
        node: Option<&'a NodeId>,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ContainerList, NativeClientError>> {
        Box::pin(self.client.list_containers(access, node, cancellation))
    }

    fn container_action<'a>(
        &'a self,
        access: &'a AccessCredential,
        target: &'a ContainerTarget,
        action: ContainerAction,
        options: ContainerActionOptions,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        Box::pin(
            self.client
                .container_action(access, target, action, options, key, cancellation),
        )
    }

    fn containers_bulk_action<'a>(
        &'a self,
        access: &'a AccessCredential,
        action: ContainerAction,
        targets: &'a [ContainerTarget],
        force: bool,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        Box::pin(self.client.containers_bulk_action(
            access,
            action,
            targets,
            force,
            key,
            cancellation,
        ))
    }

    fn list_images<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ImageList, NativeClientError>> {
        Box::pin(self.client.list_images(access, cancellation))
    }

    fn remove_image<'a>(
        &'a self,
        access: &'a AccessCredential,
        image_id: &'a str,
        nodes: &'a ImageNodes,
        force: bool,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        Box::pin(
            self.client
                .remove_image(access, image_id, nodes, force, key, cancellation),
        )
    }

    fn pull_image<'a>(
        &'a self,
        access: &'a AccessCredential,
        reference: &'a str,
        nodes: &'a [NodeId],
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        Box::pin(
            self.client
                .pull_image(access, reference, nodes, key, cancellation),
        )
    }

    fn list_volumes<'a>(
        &'a self,
        access: &'a AccessCredential,
        query: &'a VolumeQuery,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<VolumeList, NativeClientError>> {
        Box::pin(self.client.list_volumes(access, query, cancellation))
    }

    fn create_volume<'a>(
        &'a self,
        access: &'a AccessCredential,
        spec: &'a VolumeSpec,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        Box::pin(self.client.create_volume(access, spec, key, cancellation))
    }

    fn remove_volume<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: &'a NodeId,
        name: &'a VolumeName,
        key: &'a IdempotencyKey,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        Box::pin(
            self.client
                .remove_volume(access, node_id, name, key, cancellation),
        )
    }

    fn get_operation<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<OperationSnapshot, NativeClientError>> {
        Box::pin(
            self.client
                .get_operation(access, operation_id, cancellation),
        )
    }

    fn list_operation_events<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        after: u32,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Vec<OperationEvent>, NativeClientError>> {
        Box::pin(self.client.list_operation_events(
            access,
            operation_id,
            after,
            kontainer_native_client::MAX_EVENT_PAGE,
            cancellation,
        ))
    }

    fn cancel_operation<'a>(
        &'a self,
        access: &'a AccessCredential,
        operation_id: Uuid,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<OperationSnapshot, NativeClientError>> {
        Box::pin(
            self.client
                .cancel_operation(access, operation_id, cancellation),
        )
    }

    fn admit<'a>(
        &'a self,
        access: &'a AccessCredential,
        path: &'a str,
        key: &'a IdempotencyKey,
        body: &'a serde_json::Value,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<AdmittedOperation, NativeClientError>> {
        Box::pin(self.client.admit(access, path, key, body, cancellation))
    }
}

/// Map an account call's failure to its closed code plus the details the
/// connect card renders. Auth calls are never retried.
pub(crate) fn map_account_failure(failure: Traced<NativeClientError>) -> LoginFailure {
    let mut mapped = LoginFailure::from(match failure.error {
        NativeClientError::InvalidOrigin | NativeClientError::InvalidConfiguration => {
            SessionError::InvalidOrigin
        }
        NativeClientError::InvalidCredentials { .. } => SessionError::InvalidCredentials,
        NativeClientError::LoginLocked { .. } => SessionError::LoginLocked,
        NativeClientError::OwnerExists => SessionError::OwnerExists,
        NativeClientError::ValidationFailed { .. } => SessionError::ValidationFailed,
        NativeClientError::Unavailable
        | NativeClientError::Cancelled
        | NativeClientError::DeadlineExceeded
        | NativeClientError::ServiceUnavailable
        | NativeClientError::GatewayTimeout => SessionError::ControllerUnavailable,
        _ => SessionError::InvalidResponse,
    });
    match failure.error {
        NativeClientError::InvalidCredentials { attempts_remaining } => {
            mapped.attempts_remaining = Some(attempts_remaining);
        }
        NativeClientError::LoginLocked {
            retry_after_seconds,
        } => mapped.retry_after_seconds = Some(retry_after_seconds),
        NativeClientError::ValidationFailed {
            fields:
                AccountFieldErrors {
                    email,
                    password,
                    display_name,
                },
        } => {
            mapped.fields = Some(FieldErrorsView {
                email,
                password,
                display_name,
            });
        }
        _ => {}
    }
    mapped.request_id = failure.request_id;
    mapped
}

fn map_refresh_error(error: NativeClientError) -> SessionError {
    match error {
        NativeClientError::Unauthorized
        | NativeClientError::Forbidden
        | NativeClientError::NotFound => SessionError::ReloginRequired,
        NativeClientError::InvalidOrigin | NativeClientError::InvalidConfiguration => {
            SessionError::InvalidOrigin
        }
        NativeClientError::Unavailable
        | NativeClientError::Cancelled
        | NativeClientError::DeadlineExceeded
        | NativeClientError::ServiceUnavailable
        | NativeClientError::GatewayTimeout => SessionError::ControllerUnavailable,
        _ => SessionError::InvalidResponse,
    }
}

fn map_logout_error(error: NativeClientError) -> SessionError {
    // A rejected access credential does not prove the refresh credential is
    // gone, so the caller retries through rotation before treating the family
    // as dead.
    match error {
        NativeClientError::Unauthorized
        | NativeClientError::Forbidden
        | NativeClientError::NotFound => SessionError::ReloginRequired,
        NativeClientError::InvalidOrigin | NativeClientError::InvalidConfiguration => {
            SessionError::InvalidOrigin
        }
        NativeClientError::Unavailable
        | NativeClientError::Cancelled
        | NativeClientError::DeadlineExceeded
        | NativeClientError::ServiceUnavailable
        | NativeClientError::GatewayTimeout => SessionError::ControllerUnavailable,
        _ => SessionError::InvalidResponse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kontainer_native_client::FieldProblem;

    fn traced(error: NativeClientError) -> Traced<NativeClientError> {
        Traced {
            error,
            request_id: Some("req-1".to_owned()),
        }
    }

    #[test]
    fn account_failures_map_to_closed_codes_with_their_details() {
        let failure = map_account_failure(traced(NativeClientError::InvalidCredentials {
            attempts_remaining: 7,
        }));
        assert_eq!(failure.code, SessionError::InvalidCredentials);
        assert_eq!(failure.attempts_remaining, Some(7));
        assert_eq!(failure.request_id.as_deref(), Some("req-1"));

        let failure = map_account_failure(traced(NativeClientError::LoginLocked {
            retry_after_seconds: 600,
        }));
        assert_eq!(failure.code, SessionError::LoginLocked);
        assert_eq!(failure.retry_after_seconds, Some(600));

        let failure = map_account_failure(traced(NativeClientError::ValidationFailed {
            fields: AccountFieldErrors {
                email: Some(FieldProblem::Invalid),
                ..AccountFieldErrors::default()
            },
        }));
        assert_eq!(failure.code, SessionError::ValidationFailed);
        assert_eq!(
            failure.fields.and_then(|fields| fields.email),
            Some(FieldProblem::Invalid)
        );

        assert_eq!(
            map_account_failure(traced(NativeClientError::OwnerExists)),
            SessionError::OwnerExists
        );
        for transient in [
            NativeClientError::Unavailable,
            NativeClientError::DeadlineExceeded,
            NativeClientError::ServiceUnavailable,
            NativeClientError::GatewayTimeout,
        ] {
            assert_eq!(
                map_account_failure(traced(transient)),
                SessionError::ControllerUnavailable,
                "{transient:?}"
            );
        }
        assert_eq!(
            map_account_failure(traced(NativeClientError::UnexpectedStatus { status: 418 })),
            SessionError::InvalidResponse
        );
    }

    #[test]
    fn a_login_failure_serializes_only_its_present_details() {
        let serialized = serde_json::to_value(map_account_failure(traced(
            NativeClientError::InvalidCredentials {
                attempts_remaining: 3,
            },
        )))
        .unwrap();
        assert_eq!(
            serialized,
            serde_json::json!({ "code": "invalid_credentials", "attemptsRemaining": 3, "requestId": "req-1" })
        );
        let serialized = serde_json::to_value(LoginFailure::from(SessionError::ProfileConflict))
            .expect("serializes");
        assert_eq!(
            serialized,
            serde_json::json!({ "code": "profile_conflict" })
        );
    }

    #[test]
    fn refresh_and_logout_rejections_mean_relogin_while_transients_stay_distinct() {
        for rejected in [
            NativeClientError::Unauthorized,
            NativeClientError::Forbidden,
            NativeClientError::NotFound,
        ] {
            assert_eq!(
                map_refresh_error(rejected),
                SessionError::ReloginRequired,
                "{rejected:?}"
            );
            assert_eq!(
                map_logout_error(rejected),
                SessionError::ReloginRequired,
                "{rejected:?}"
            );
        }
        for transient in [
            NativeClientError::Unavailable,
            NativeClientError::DeadlineExceeded,
            NativeClientError::ServiceUnavailable,
        ] {
            assert_eq!(
                map_refresh_error(transient),
                SessionError::ControllerUnavailable,
                "{transient:?}"
            );
            assert_eq!(
                map_logout_error(transient),
                SessionError::ControllerUnavailable,
                "{transient:?}"
            );
        }
    }
}
