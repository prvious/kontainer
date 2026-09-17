use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use loco_rs::{app::AppContext, controller::Routes};
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use utoipa::ToSchema;

use crate::health::check_readiness;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub ok: bool,
}

fn response(ok: bool) -> Response {
    (
        if ok {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        [(header::CACHE_CONTROL, "no-store")],
        Json(HealthResponse { ok }),
    )
        .into_response()
}

#[utoipa::path(get, path = "/health/live", responses((status = 200, body = HealthResponse)))]
pub(crate) async fn live() -> Response {
    response(true)
}

// The supervisor admits no HTTP requests until static configuration and RPC TLS
// preparation succeed. Reaching this handler therefore proves startup completed.
#[utoipa::path(get, path = "/health/startup", responses((status = 200, body = HealthResponse)))]
pub(crate) async fn startup() -> Response {
    response(true)
}

#[utoipa::path(get, path = "/health/ready", responses(
    (status = 200, body = HealthResponse),
    (status = 503, body = HealthResponse)
))]
pub(crate) async fn ready(State(context): State<AppContext>) -> Response {
    let Some(shutdown) = context.shared_store.get::<CancellationToken>() else {
        return response(false);
    };
    response(check_readiness::execute(&context.db, &shutdown).await)
}

/// Requests already admitted may drain; new application requests must not start.
pub(crate) async fn admit_request(
    State(shutdown): State<CancellationToken>,
    request: Request,
    next: Next,
) -> Response {
    if shutdown.is_cancelled()
        && !matches!(
            request.uri().path(),
            "/health/live" | "/health/startup" | "/health/ready"
        )
    {
        return response(false);
    }
    next.run(request).await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/health/live", live)
        .get("/health/startup", startup)
        .get("/health/ready", ready)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use axum::{
        Router,
        body::{Body, to_bytes},
        middleware,
        routing::get,
    };
    use loco_rs::{app::Hooks, config::Config, environment::Environment};
    use sea_orm::DatabaseConnection;
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use tower::ServiceExt;

    fn context() -> AppContext {
        AppContext::builder(
            Environment::Test,
            DatabaseConnection::default(),
            Config::from_folder(&Environment::Test, Path::new("config")).unwrap(),
        )
        .build()
    }

    #[tokio::test]
    async fn probes_are_minimal_uncacheable_and_distinguish_readiness_during_drain() {
        let context = App::after_context(context()).await.unwrap();
        let shutdown = context.shared_store.get::<CancellationToken>().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = calls.clone();
        let router = Router::new()
            .route("/health/live", get(live))
            .route("/health/startup", get(startup))
            .route("/health/ready", get(ready))
            .route(
                "/application",
                get(move || {
                    handler_calls.fetch_add(1, Ordering::SeqCst);
                    async { "ok" }
                }),
            )
            .with_state(context)
            .layer(middleware::from_fn_with_state(
                shutdown.clone(),
                admit_request,
            ));

        for draining in [false, true] {
            if draining {
                shutdown.cancel();
            }
            for (path, expected_ok) in [
                ("/health/live", true),
                ("/health/startup", true),
                ("/health/ready", false),
            ] {
                let response = router
                    .clone()
                    .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(
                    response.status(),
                    if expected_ok {
                        StatusCode::OK
                    } else {
                        StatusCode::SERVICE_UNAVAILABLE
                    }
                );
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                let body = to_bytes(response.into_body(), 256).await.unwrap();
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                    serde_json::json!({"ok": expected_ok})
                );
            }
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/application")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if draining {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::OK
                }
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn missing_lifecycle_state_fails_readiness_closed() {
        assert_eq!(
            ready(State(context())).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn app_registers_only_the_intentional_probe_routes() {
        let routes = App::routes(&context()).collect();
        for path in ["/health/live", "/health/startup", "/health/ready"] {
            assert!(routes.iter().any(|route| route.uri == path));
        }
        for path in ["/_health", "/_ping", "/_readiness"] {
            assert!(!routes.iter().any(|route| route.uri == path));
        }
    }
}
