use loco_rs::prelude::*;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};

use super::operations::{OperationResponse, OperationStateResponse};

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
        (status = 503, description = "Controller draining new work; probes remain available", body = super::health::HealthResponse)
    )
)]
async fn show() -> Result<Response> {
    format::json(StatusResponse::current())
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
    ))
)]
pub(crate) struct ApiDoc;

async fn openapi() -> Result<Response> {
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
    }
}
