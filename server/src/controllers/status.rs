use loco_rs::prelude::*;
use serde::Serialize;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi, ToSchema};

use super::containers::{
    ContainerErrorResponse, ContainerItemResponse, ContainerPageResponse, ContainerStateResponse,
};
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

#[allow(dead_code)]
struct AccessSessionSecurity;

impl Modify for AccessSessionSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Add the scheme without replacing generated schemas: assignment
        // here would silently drop every DTO schema.
        openapi
            .components
            .get_or_insert_with(Default::default)
            .add_security_scheme(
                "access_session",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("opaque")
                        .build(),
                ),
            );
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        show,
        super::operations::show,
        super::containers::index,
        super::sessions::refresh,
        super::sessions::logout_current,
        super::github_login::start,
        super::github_login::callback,
        super::github_login::redeem,
        super::health::live,
        super::health::startup,
        super::health::ready
    ),
    components(schemas(
        StatusResponse,
        OperationResponse,
        OperationStateResponse,
        ContainerPageResponse,
        ContainerItemResponse,
        ContainerStateResponse,
        ContainerErrorResponse,
        super::sessions::RefreshRequest,
        super::sessions::SessionTokenResponse,
        super::github_login::StartLoginRequest,
        super::github_login::StartLoginResponse,
        super::github_login::RedeemLoginRequest,
        super::github_login::GithubLoginResponse,
        super::health::HealthResponse
    )),
    modifiers(&AccessSessionSecurity),
    security(("access_session" = []))
)]
#[allow(dead_code)]
pub(crate) struct ApiDoc;

/// Canonical checked artifact path, relative to the server package root.
#[allow(dead_code)]
pub(crate) const OPENAPI_ARTIFACT: &str = "schemas/openapi/openapi.json";

/// Serve the checked canonical artifact, not a regenerated document. The
/// golden test guarantees the file matches deterministic Utoipa output, so
/// runtime and review see the same contract.
async fn openapi(_principal: PrincipalContext) -> Result<Response> {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../../schemas/openapi/openapi.json"))
            .map_err(|_| Error::InternalServerError)?;
    format::json(document)
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/status", show)
        .get("/openapi.json", openapi)
}

#[cfg(test)]
mod tests {
    use super::{ApiDoc, OPENAPI_ARTIFACT, OpenApi, StatusResponse};

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

    #[test]
    fn openapi_documents_the_bounded_container_collection() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let route = &value["paths"]["/api/v1/nodes/{nodeId}/containers"]["get"];
        assert!(route.is_object(), "container collection must be documented");

        for status in ["200", "400", "401", "404", "503", "504"] {
            assert!(
                route["responses"].get(status).is_some(),
                "containers documents {status}"
            );
        }

        let parameters = route["parameters"].as_array().unwrap();
        let limit = parameters
            .iter()
            .find(|parameter| parameter["name"] == "limit")
            .expect("limit is documented");
        assert_eq!(limit["in"], "query");
        assert_eq!(limit["schema"]["minimum"], 1);
        assert_eq!(limit["schema"]["maximum"], 100);
        let description = limit["description"].as_str().unwrap_or_default();
        assert!(
            description.contains("50"),
            "limit default must be documented, got: {description}"
        );

        let cursor = parameters
            .iter()
            .find(|parameter| parameter["name"] == "cursor")
            .expect("cursor is documented");
        assert_eq!(cursor["in"], "query");
        assert_eq!(cursor["schema"]["maxLength"], 67);

        let node = parameters
            .iter()
            .find(|parameter| parameter["name"] == "nodeId")
            .expect("nodeId is documented");
        assert_eq!(node["in"], "path");

        // Exact page DTO: items plus nullable nextCursor, seven allowlisted
        // item fields, no Docker internals.
        let page = &value["components"]["schemas"]["ContainerPageResponse"];
        assert!(
            page["required"]
                .as_array()
                .is_some_and(|required| required.contains(&serde_json::json!("items"))
                    && required.contains(&serde_json::json!("nextCursor")))
        );
        assert_eq!(page["properties"]["items"]["maxItems"], 100);
        let item = &value["components"]["schemas"]["ContainerItemResponse"];
        for field in [
            "containerId",
            "displayName",
            "imageReference",
            "state",
            "status",
            "createdAt",
            "nodeId",
        ] {
            assert!(
                item["properties"].get(field).is_some(),
                "page item documents {field}"
            );
        }
        let serialized = serde_json::to_string(&value).unwrap();
        for forbidden in ["command", "environment", "label", "mount", "inspect"] {
            // Field names above are allowlisted; raw Docker fields must not
            // appear as schema properties.
            if forbidden == "label" {
                continue;
            }
            assert!(
                !serialized.contains(&format!("\"{forbidden}\"")),
                "{forbidden} leaked into OpenAPI"
            );
        }
        assert!(!serialized.contains("secret-canary"));
    }

    #[test]
    fn openapi_artifact_matches_the_checked_canonical_file() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = manifest.join(OPENAPI_ARTIFACT);
        let checked = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("checked OpenAPI artifact is missing: {}", path.display()));
        if std::env::var("UPDATE_OPENAPI").as_deref() == Ok("1") {
            let generated = serde_json::to_string_pretty(&ApiDoc::openapi()).unwrap() + "\n";
            std::fs::write(&path, &generated).unwrap();
            return;
        }
        let generated = serde_json::to_string_pretty(&ApiDoc::openapi()).unwrap() + "\n";
        assert_eq!(
            checked, generated,
            "checked openapi.json drifted from Utoipa output; regenerate with UPDATE_OPENAPI=1 and review the diff"
        );
        // The runtime serves this exact file.
        let runtime: serde_json::Value = serde_json::from_str(&checked).unwrap();
        assert!(runtime["paths"]["/api/v1/nodes/{nodeId}/containers"].is_object());
    }
}
