use loco_rs::prelude::*;
use serde::Serialize;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi, ToSchema};

use super::operations::{OperationResponse, OperationStateResponse};
use crate::auth::domain::PrincipalContext;

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub architecture: &'static str,
}

impl StatusResponse {
    fn current() -> Self {
        Self {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
            architecture: std::env::consts::ARCH,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/status",
    responses(
        (status = 200, description = "Controller build and liveness only; use /health/ready for readiness", body = StatusResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Controller draining new work or session persistence unavailable; the body is either HealthResponse or a generic error")
    )
)]
async fn show(_principal: PrincipalContext) -> Result<Response> {
    format::json(StatusResponse::current())
}

struct AccessSessionSecurity;

impl Modify for AccessSessionSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi.components = Some(
            utoipa::openapi::ComponentsBuilder::new()
                .security_scheme(
                    "access_session",
                    SecurityScheme::Http(
                        HttpBuilder::new()
                            .scheme(HttpAuthScheme::Bearer)
                            .bearer_format("opaque")
                            .build(),
                    ),
                )
                .build(),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        show,
        super::operations::show,
        super::sessions::refresh,
        super::sessions::logout_current,
        super::health::live,
        super::health::startup,
        super::health::ready
    ),
    components(schemas(
        StatusResponse,
        OperationResponse,
        OperationStateResponse,
        super::sessions::RefreshRequest,
        super::sessions::SessionTokenResponse,
        super::health::HealthResponse
    )),
    modifiers(&AccessSessionSecurity),
    security(("access_session" = []))
)]
pub(crate) struct ApiDoc;

async fn openapi(_principal: PrincipalContext) -> Result<Response> {
    format::json(ApiDoc::openapi())
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/status", show)
        .get("/openapi.json", openapi)
}

#[cfg(test)]
mod tests {
    use super::{ApiDoc, OpenApi, StatusResponse};

    #[test]
    fn status_reports_the_current_controller_build() {
        let response = StatusResponse::current();

        assert_eq!(response.status, "ok");
        assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
        assert!(!response.architecture.is_empty());
    }

    #[test]
    fn openapi_contains_the_versioned_status_contract() {
        let document = ApiDoc::openapi();

        assert!(document.paths.paths.contains_key("/api/v1/status"));
        for path in ["/health/live", "/health/startup", "/health/ready"] {
            assert!(document.paths.paths.contains_key(path));
        }
        assert!(
            document
                .paths
                .paths
                .contains_key("/api/v1/operations/{operation_id}")
        );

        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for path in ["/api/v1/status", "/api/v1/operations/{operation_id}"] {
            let responses = &value["paths"][path]["get"]["responses"];
            assert!(
                responses.get("401").is_some(),
                "{path} documents its access-session challenge"
            );
        }
        let operation_responses =
            &value["paths"]["/api/v1/operations/{operation_id}"]["get"]["responses"];
        assert!(
            operation_responses["404"]["description"]
                .as_str()
                .is_some_and(|description| description.contains("not visible")),
            "polling documents the anti-oracle 404"
        );
    }
}
