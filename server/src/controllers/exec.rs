//! `GET /api/v1/ws/exec`: an interactive shell in one container over the binary WebSocket.

use std::sync::Arc;

use axum::{
    Extension,
    extract::{State, ws::WebSocketUpgrade},
    http::StatusCode,
    response::Response,
};
use kontainer_stream_transport::StreamAdmissionError;
use loco_rs::{app::AppContext, controller::Routes};

use super::{admission::admission_error, services::shutdown_token};
use crate::{
    auth::domain::{PrincipalContext, ValidatedSession},
    exec::{ExecStreams, session::ExecSession},
    nodes::agents::SharedAgentDirectory,
};

#[utoipa::path(
    get,
    path = "/api/v1/ws/exec",
    operation_id = "exec_stream",
    responses(
        (status = 101, description = "Upgraded to the `kontainer.stream.v1` subprotocol. The client sends one `ExecOpen`, then `ExecInput`, `ExecResize`, and `ExecClose`; the server answers `ExecStarted` or one `ExecError`, then `ExecOutput` and one `ExecExit`. An expired credential closes with 4401."),
        (status = 400, description = "`invalid_request`: not a WebSocket upgrade, or without the stream subprotocol", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential; refused before the upgrade"),
        (status = 429, description = "`limit_reached`: this session already has four exec sessions open", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`unavailable` while the controller drains", body = super::admission::AdmissionErrorResponse)
    )
)]
pub(crate) async fn stream(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    Extension(session): Extension<ValidatedSession>,
    websocket: WebSocketUpgrade,
) -> Response {
    let Some(streams) = context.shared_store.get::<ExecStreams>() else {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let Some(slot) = streams.sessions.try_acquire(session.family_id) else {
        return admission_error(StatusCode::TOO_MANY_REQUESTS, "limit_reached");
    };
    let agents = context
        .shared_store
        .get::<SharedAgentDirectory>()
        .map(|directory| directory.0);
    match streams.server.admit(&websocket) {
        Ok(admission) => admission.upgrade_with(
            websocket,
            Arc::new(ExecSession::new(
                context.db.clone(),
                agents,
                session,
                streams.timing,
                shutdown_token(&context),
                slot,
            )),
        ),
        Err(StreamAdmissionError::UnsupportedSubprotocol) => {
            admission_error(StatusCode::BAD_REQUEST, "invalid_request")
        }
        Err(StreamAdmissionError::AtCapacity) => {
            admission_error(StatusCode::TOO_MANY_REQUESTS, "limit_reached")
        }
        Err(StreamAdmissionError::ShuttingDown) => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
        }
    }
}

pub fn routes() -> Routes {
    Routes::new().get("/ws/exec", stream)
}
