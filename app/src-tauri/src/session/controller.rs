//! Minimal controller port over the UI-neutral native client.
//!
//! The port exists so the custody state machine can be tested with scripted
//! outcomes and so every transport failure maps to exactly one closed session
//! code at the boundary. It adds no endpoint of its own.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use kontainer_native_client::{
    AccessCredential, ContainerPage, ContainerPageRequest, LoginVerifier, NativeClient,
    NativeClientConfig, NativeClientError, NodeId, RefreshCredential,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::errors::SessionError;

/// Boxed future used by the object-safe controller trait.
pub(crate) type ControllerFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A started login attempt, reduced to what custody needs.
pub(crate) struct LoginStart {
    pub attempt_id: Uuid,
    pub authorize_url: String,
    pub expires_at_unix_ms: i64,
}

/// The credential pair issued by redemption or rotation.
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
    pub github_login: String,
    pub issued: IssuedCredentials,
}

/// Redemption either completed or is still waiting for the browser step.
pub(crate) enum Redemption {
    Ready(RedeemedSession),
    Pending,
}

/// The one-per-origin controller surface custody depends on.
pub(crate) trait Controller: Send + Sync + 'static {
    /// The validated origin this controller is bound to.
    fn canonical_origin(&self) -> &str;

    fn start_login<'a>(
        &'a self,
        verifier: &'a LoginVerifier,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<LoginStart, SessionError>>;

    fn redeem_login<'a>(
        &'a self,
        attempt_id: Uuid,
        verifier: &'a LoginVerifier,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Redemption, SessionError>>;

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

    fn discover_node<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Option<NodeId>, NativeClientError>>;

    fn list_containers<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: &'a NodeId,
        page: &'a ContainerPageRequest,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ContainerPage, NativeClientError>>;
}

/// Builds a controller for a validated controller origin.
pub(crate) trait ControllerFactory: Send + Sync + 'static {
    fn for_origin(&self, origin: &str) -> Result<Arc<dyn Controller>, SessionError>;
}

/// Production factory: validates the origin, then builds the native client.
pub(crate) struct NativeControllerFactory;

impl ControllerFactory for NativeControllerFactory {
    fn for_origin(&self, origin: &str) -> Result<Arc<dyn Controller>, SessionError> {
        let client = NativeClient::new(NativeClientConfig::new(origin))
            .map_err(|_| SessionError::InvalidOrigin)?;
        Ok(Arc::new(NativeController { client }))
    }
}

struct NativeController {
    client: NativeClient,
}

impl Controller for NativeController {
    fn canonical_origin(&self) -> &str {
        self.client.controller_origin()
    }

    fn start_login<'a>(
        &'a self,
        verifier: &'a LoginVerifier,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<LoginStart, SessionError>> {
        Box::pin(async move {
            self.client
                .start_login(verifier, cancellation)
                .await
                .map(|attempt| LoginStart {
                    attempt_id: attempt.attempt_id,
                    authorize_url: attempt.authorize_url.as_str().to_owned(),
                    expires_at_unix_ms: attempt.expires_at.timestamp_millis(),
                })
                .map_err(map_login_error)
        })
    }

    fn redeem_login<'a>(
        &'a self,
        attempt_id: Uuid,
        verifier: &'a LoginVerifier,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Redemption, SessionError>> {
        Box::pin(async move {
            match self
                .client
                .redeem_login(attempt_id, verifier, cancellation)
                .await
            {
                Ok(session) => Ok(Redemption::Ready(RedeemedSession {
                    cluster_id: session.cluster_id,
                    principal_id: session.principal_id,
                    github_login: session.github_login,
                    issued: IssuedCredentials {
                        access: session.credentials.access,
                        refresh: session.credentials.refresh,
                        access_expires_at_unix_ms: session
                            .credentials
                            .access_expires_at
                            .timestamp_millis(),
                        refresh_expires_at_unix_ms: session
                            .credentials
                            .refresh_expires_at
                            .timestamp_millis(),
                    },
                })),
                Err(NativeClientError::Conflict) => Ok(Redemption::Pending),
                Err(error) => Err(map_redeem_error(error)),
            }
        })
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

    fn discover_node<'a>(
        &'a self,
        access: &'a AccessCredential,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<Option<NodeId>, NativeClientError>> {
        Box::pin(self.client.discover_node(access, cancellation))
    }

    fn list_containers<'a>(
        &'a self,
        access: &'a AccessCredential,
        node_id: &'a NodeId,
        page: &'a ContainerPageRequest,
        cancellation: &'a CancellationToken,
    ) -> ControllerFuture<'a, Result<ContainerPage, NativeClientError>> {
        Box::pin(
            self.client
                .list_containers(access, node_id, page, cancellation),
        )
    }
}

fn map_login_error(error: NativeClientError) -> SessionError {
    match error {
        NativeClientError::InvalidOrigin | NativeClientError::InvalidConfiguration => {
            SessionError::InvalidOrigin
        }
        NativeClientError::Cancelled => SessionError::LoginCancelled,
        NativeClientError::Unavailable
        | NativeClientError::DeadlineExceeded
        | NativeClientError::ServiceUnavailable
        | NativeClientError::GatewayTimeout => SessionError::ControllerUnavailable,
        NativeClientError::ProviderUnavailable => SessionError::ProviderUnavailable,
        _ => SessionError::InvalidResponse,
    }
}

fn map_redeem_error(error: NativeClientError) -> SessionError {
    match error {
        NativeClientError::Cancelled => SessionError::LoginCancelled,
        NativeClientError::Forbidden | NativeClientError::NotFound => SessionError::LoginDenied,
        NativeClientError::ProviderUnavailable => SessionError::ProviderUnavailable,
        NativeClientError::Unavailable
        | NativeClientError::DeadlineExceeded
        | NativeClientError::ServiceUnavailable
        | NativeClientError::GatewayTimeout => SessionError::ControllerUnavailable,
        _ => SessionError::InvalidResponse,
    }
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

    #[test]
    fn login_failures_map_to_closed_codes() {
        assert_eq!(
            map_login_error(NativeClientError::InvalidOrigin),
            SessionError::InvalidOrigin
        );
        assert_eq!(
            map_login_error(NativeClientError::Cancelled),
            SessionError::LoginCancelled
        );
        for transient in [
            NativeClientError::Unavailable,
            NativeClientError::DeadlineExceeded,
            NativeClientError::ServiceUnavailable,
            NativeClientError::GatewayTimeout,
        ] {
            assert_eq!(
                map_login_error(transient),
                SessionError::ControllerUnavailable,
                "{transient:?}"
            );
        }
        assert_eq!(
            map_login_error(NativeClientError::ProviderUnavailable),
            SessionError::ProviderUnavailable
        );
        assert_eq!(
            map_login_error(NativeClientError::UnexpectedStatus { status: 418 }),
            SessionError::InvalidResponse
        );
    }

    #[test]
    fn redemption_failures_map_and_conflict_is_handled_before_mapping() {
        assert_eq!(
            map_redeem_error(NativeClientError::Cancelled),
            SessionError::LoginCancelled
        );
        for denied in [NativeClientError::Forbidden, NativeClientError::NotFound] {
            assert_eq!(
                map_redeem_error(denied),
                SessionError::LoginDenied,
                "{denied:?}"
            );
        }
        assert_eq!(
            map_redeem_error(NativeClientError::ProviderUnavailable),
            SessionError::ProviderUnavailable
        );
        assert_eq!(
            map_redeem_error(NativeClientError::GatewayTimeout),
            SessionError::ControllerUnavailable
        );
        // `Conflict` never reaches this mapping: the controller port turns it
        // into `Redemption::Pending` so the login loop polls again.
        assert_eq!(
            map_redeem_error(NativeClientError::Conflict),
            SessionError::InvalidResponse
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
