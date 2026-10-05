//! `GET /api/v1/ws/logs`: one service's logs over the binary WebSocket transport.

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
    logs::{LogStreams, session::LogsSession},
    services::actions::list_services::MANAGER_READ_DEADLINE,
    swarm::engine::{ManagerError, SharedManagerEngine},
};

#[utoipa::path(
    get,
    path = "/api/v1/ws/logs",
    operation_id = "logs_stream",
    responses(
        (status = 101, description = "Upgraded to the `kontainer.stream.v1` subprotocol. The client sends one `LogsOpen`; the server answers with `LogLine`, `LogGap`, and a final `LogEnd`, and closes with 4401 when the access credential expires."),
        (status = 400, description = "`invalid_request`: not a WebSocket upgrade, or without the stream subprotocol", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential; refused before the upgrade"),
        (status = 429, description = "`limit_reached`: this session already has eight log connections open", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`manager_unavailable` when the Swarm manager doesn't answer, `not_a_manager`, or `unavailable` while the controller drains", body = super::admission::AdmissionErrorResponse)
    )
)]
pub(crate) async fn stream(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    Extension(session): Extension<ValidatedSession>,
    websocket: WebSocketUpgrade,
) -> Response {
    let Some(streams) = context.shared_store.get::<LogStreams>() else {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let Some(manager) = context.shared_store.get::<SharedManagerEngine>() else {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "not_a_manager");
    };
    // A manager that doesn't answer refuses the upgrade, so a client counts the attempt as failed
    // and backs off, instead of connecting only to be told the manager is gone.
    let cancellation = shutdown_token(&context);
    match tokio::time::timeout(MANAGER_READ_DEADLINE, manager.0.swarm_status(&cancellation)).await {
        Ok(Ok(_)) => {}
        Ok(Err(ManagerError::NotAManager)) => {
            return admission_error(StatusCode::SERVICE_UNAVAILABLE, "not_a_manager");
        }
        _ => return admission_error(StatusCode::SERVICE_UNAVAILABLE, "manager_unavailable"),
    }
    let Some(slot) = streams.connections.try_acquire(session.family_id) else {
        return admission_error(StatusCode::TOO_MANY_REQUESTS, "limit_reached");
    };
    match streams.server.admit(&websocket) {
        Ok(admission) => admission.upgrade_with(
            websocket,
            Arc::new(LogsSession::new(
                manager.0,
                session,
                streams.liveness,
                cancellation,
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
    Routes::new().get("/ws/logs", stream)
}
