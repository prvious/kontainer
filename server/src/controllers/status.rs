use loco_rs::prelude::*;
use serde::Serialize;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi, ToSchema};

use super::containers::{
    BulkActionRequest, ContainerActionRequest, ContainerListResponse, ContainerResponse,
    ContainerStateResponse, TooManyContainersResponse,
};
use super::operations::{OperationResponse, OperationStateResponse};
use crate::auth::domain::PrincipalContext;

mod deep;

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
    operation_id = "status",
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
        deep::show,
        super::operations::show,
        super::operations::events,
        super::operations::cancel,
        super::operations::admit_noop,
        super::nodes::index,
        super::nodes::samples,
        super::nodes::cluster_samples,
        super::nodes::cluster_summary,
        super::logs::stream,
        super::exec::stream,
        super::settings::show,
        super::settings::update,
        super::containers::index,
        super::containers::action,
        super::containers::bulk,
        super::images::index,
        super::images::remove,
        super::images::pull,
        super::volumes::index,
        super::volumes::create,
        super::volumes::remove,
        super::networks::index,
        super::networks::show,
        super::networks::create,
        super::networks::remove,
        super::services::index,
        super::services::show,
        super::services::action,
        super::stacks::index,
        super::stacks::plan,
        super::stacks::remove,
        super::sessions::refresh,
        super::sessions::logout_current,
        super::password_auth::bootstrap,
        super::password_auth::register,
        super::password_auth::password_login,
        super::account::current,
        super::github_login::start,
        super::github_login::callback,
        super::github_login::redeem,
        super::health::live,
        super::health::startup,
        super::health::ready
    ),
    components(schemas(
        StatusResponse,
        deep::DeepStatusResponse,
        deep::BuildResponse,
        deep::MigrationState,
        deep::DatabaseState,
        OperationResponse,
        OperationStateResponse,
        super::operations::OperationEventResponse,
        super::operations::OperationEventsResponse,
        super::admission::AdmittedResponse,
        super::admission::AdmissionErrorResponse,
        ContainerListResponse,
        ContainerResponse,
        ContainerStateResponse,
        TooManyContainersResponse,
        ContainerActionRequest,
        BulkActionRequest,
        super::images::ImageListResponse,
        super::images::ImageResponse,
        super::admission::FieldErrorResponse,
        super::images::RemoveImageRequest,
        super::images::PullImageRequest,
        super::volumes::VolumeListResponse,
        super::volumes::VolumeResponse,
        super::volumes::CreateVolumeRequest,
        super::networks::NetworkListResponse,
        super::networks::NetworkResponse,
        super::networks::NetworkContainersResponse,
        super::networks::AttachedContainerResponse,
        super::networks::CreateNetworkRequest,
        super::services::ServiceListResponse,
        super::services::ServiceResponse,
        super::services::ServiceDetailResponse,
        super::services::TaskResponse,
        super::services::QuorumUnavailableResponse,
        super::services::ServiceActionRequest,
        super::stacks::StackListResponse,
        super::stacks::StackResponse,
        super::stacks::StackPlanResponse,
        crate::nodes::fan_out::NodeOutcomeResponse,
        crate::nodes::fan_out::NodeOutcomeStatus,
        super::nodes::NodeListResponse,
        super::nodes::NodeResponse,
        super::nodes::AgentStatusResponse,
        super::nodes::NodeHealthResponse,
        super::nodes::NodeRoleResponse,
        super::nodes::NodeAvailabilityResponse,
        super::nodes::SwarmNodeStatusResponse,
        super::nodes::ClusterSummaryResponse,
        super::nodes::QuorumStateResponse,
        super::nodes::NodeSampleResponse,
        super::nodes::SampleSeriesResponse,
        super::nodes::SamplePointResponse,
        super::nodes::NodeErrorResponse,
        super::settings::SettingsResponse,
        super::settings::SettingsPatchRequest,
        super::sessions::RefreshRequest,
        super::sessions::SessionTokenResponse,
        super::password_auth::BootstrapResponse,
        super::password_auth::RegisterRequest,
        super::password_auth::PasswordLoginRequest,
        super::password_auth::PasswordSessionResponse,
        super::password_auth::ValidationErrorResponse,
        super::password_auth::InvalidCredentialsResponse,
        super::password_auth::LockedResponse,
        super::account::AccountResponse,
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
        .get("/status/deep", deep::show)
        .get("/openapi.json", openapi)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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
    fn deep_status_openapi_has_exact_closed_fields() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let route = &document["paths"]["/api/v1/status/deep"]["get"];
        assert!(route["responses"]["200"].is_object());
        assert!(route["responses"]["401"].is_object());
        assert!(route["responses"]["503"].is_object());
        assert_eq!(route["operationId"], "deep_status");
        assert_eq!(
            route["security"],
            serde_json::json!([{"access_session": []}])
        );

        let schemas = &document["components"]["schemas"];
        let response = &schemas["DeepStatusResponse"];
        let fields: BTreeSet<&str> = response["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            fields,
            BTreeSet::from([
                "build",
                "migration",
                "database",
                "nodesOnline",
                "nodesTotal"
            ])
        );
        assert_eq!(response["additionalProperties"], false);
        assert_eq!(response["required"].as_array().unwrap().len(), 5);
        let build = &schemas["BuildResponse"];
        assert_eq!(build["additionalProperties"], false);
        assert_eq!(build["properties"].as_object().unwrap().len(), 1);
        assert!(build["properties"]["version"].is_object());
        assert_eq!(
            schemas["MigrationState"]["enum"],
            serde_json::json!(["current", "unknown"])
        );
        assert_eq!(
            schemas["DatabaseState"]["enum"],
            serde_json::json!(["reachable", "unreachable", "timeout"])
        );
    }

    #[test]
    fn membership_metrics_and_settings_routes_are_documented() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for (path, method, operation) in [
            ("/api/v1/nodes", "get", "list_nodes"),
            ("/api/v1/nodes/{nodeId}/samples", "get", "node_samples"),
            ("/api/v1/cluster/samples", "get", "cluster_samples"),
            ("/api/v1/cluster/summary", "get", "cluster_summary"),
            ("/api/v1/settings", "get", "get_settings"),
            ("/api/v1/settings", "patch", "update_settings"),
        ] {
            let route = &document["paths"][path][method];
            assert_eq!(route["operationId"], operation, "{path}");
            assert_eq!(
                route["security"],
                serde_json::json!([{"access_session": []}])
            );
            assert!(route["responses"]["401"].is_object(), "{path}");
        }
        assert!(document["paths"]["/api/v1/nodes/configured"].is_null());
        let schemas = &document["components"]["schemas"];
        assert_eq!(
            schemas["AgentStatusResponse"]["enum"],
            serde_json::json!(["online", "degraded", "offline", "no-agent"])
        );
        assert_eq!(
            schemas["NodeHealthResponse"]["enum"],
            serde_json::json!(["active", "degraded", "down"])
        );
        let node = schemas["NodeResponse"]["required"].as_array().unwrap();
        for nullable in ["availability", "address", "agentVersion", "lastCheckInAt"] {
            assert!(
                node.contains(&serde_json::json!(nullable)),
                "{nullable} is nullable, not optional"
            );
        }
        assert!(
            schemas["ClusterSummaryResponse"]["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("cpuPercent"))
        );
        let point = schemas["SamplePointResponse"]["required"]
            .as_array()
            .unwrap();
        assert!(
            point.contains(&serde_json::json!("cpuPercent")),
            "nullable, not optional"
        );
    }

    #[test]
    fn openapi_documents_the_cluster_container_collection_and_actions() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        assert!(value["paths"]["/api/v1/nodes/{nodeId}/containers"].is_null());
        let list = &value["paths"]["/api/v1/containers"]["get"];
        for status in ["200", "400", "401", "404", "413", "503"] {
            assert!(
                list["responses"].get(status).is_some(),
                "list documents {status}"
            );
        }
        for (path, operation) in [
            (
                "/api/v1/containers/{nodeId}/{containerId}/actions/{action}",
                "container_action",
            ),
            ("/api/v1/containers/actions/bulk", "containers_bulk_action"),
        ] {
            let route = &value["paths"][path]["post"];
            assert_eq!(route["operationId"], operation);
            assert!(route["responses"]["202"].is_object());
            assert!(route["responses"]["409"].is_object());
        }
        let item = &value["components"]["schemas"]["ContainerResponse"];
        let fields: std::collections::BTreeSet<_> = item["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for forbidden in ["command", "labels", "env", "mounts"] {
            assert!(!fields.contains(forbidden), "{forbidden} never crosses");
        }
        assert!(fields.contains("nodeName") && fields.contains("ports"));
    }

    #[test]
    fn operation_ids_are_unique() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let mut seen = BTreeSet::new();
        for (path, item) in value["paths"].as_object().unwrap() {
            for (method, operation) in item.as_object().unwrap() {
                let id = operation["operationId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                assert!(
                    seen.insert(id.clone()),
                    "{method} {path} reuses operationId {id}"
                );
            }
        }
        assert!(seen.contains("status") && seen.contains("operation"));
    }

    #[test]
    fn password_auth_routes_are_documented_without_a_session_requirement() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for (path, method) in [
            ("/api/v1/auth/bootstrap", "get"),
            ("/api/v1/auth/register", "post"),
            ("/api/v1/auth/password/login", "post"),
        ] {
            let route = &value["paths"][path][method];
            assert!(route.is_object(), "{path} is documented");
            assert_eq!(
                route["security"],
                serde_json::json!([{}]),
                "{path} is anonymous"
            );
        }
        let login = &value["paths"]["/api/v1/auth/password/login"]["post"]["responses"];
        for status in ["200", "400", "401", "429", "503"] {
            assert!(login.get(status).is_some(), "login documents {status}");
        }
        assert!(value["paths"]["/api/v1/auth/session"]["get"].is_object());
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
        assert!(runtime["paths"]["/api/v1/containers"].is_object());
    }
}
