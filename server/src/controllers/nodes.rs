//! Discovery of the controller's one configured agent node.

use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use loco_rs::{app::AppContext, controller::Routes};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{auth::domain::PrincipalContext, rpc::SharedAgentClient};

#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfiguredNodeResponse {
    /// The configured node ID, or null when no agent client is configured.
    #[schema(required = true)]
    node_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/nodes/configured",
    operation_id = "configured_node",
    responses(
        (status = 200, description = "The one configured node, without contacting the agent", body = ConfiguredNodeResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Controller draining new work or session persistence unavailable")
    ),
    security(("access_session" = []))
)]
pub(crate) async fn configured(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    let node_id = context
        .shared_store
        .get::<SharedAgentClient>()
        .map(|client| client.0.node_id().to_owned());
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(ConfiguredNodeResponse { node_id }),
    )
        .into_response()
}

pub fn routes() -> Routes {
    Routes::new().get("/nodes/configured", configured)
}

#[cfg(test)]
mod tests {
    use super::ConfiguredNodeResponse;

    #[test]
    fn discovery_body_contains_only_the_configured_id() {
        assert_eq!(
            serde_json::to_value(ConfiguredNodeResponse {
                node_id: Some("node-1".to_owned()),
            })
            .unwrap(),
            serde_json::json!({ "nodeId": "node-1" })
        );
        assert_eq!(
            serde_json::to_value(ConfiguredNodeResponse { node_id: None }).unwrap(),
            serde_json::json!({ "nodeId": null })
        );
    }
}
