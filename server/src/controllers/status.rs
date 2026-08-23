use loco_rs::prelude::*;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};

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
    responses((status = 200, description = "Controller is running", body = StatusResponse))
)]
async fn show() -> Result<Response> {
    format::json(StatusResponse::current())
}

#[derive(OpenApi)]
#[openapi(paths(show), components(schemas(StatusResponse)))]
struct ApiDoc;

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
    }
}
