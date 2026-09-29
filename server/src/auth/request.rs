//! Access-session request boundary.
//!
//! [`require_access_session`] validates the single `Authorization: Bearer …`
//! value against live persisted session state before the handler runs and
//! inserts the resulting [`PrincipalContext`] into request extensions.
//! [`PrincipalContext`] extraction fails closed when the middleware did not
//! run, so a route without the middleware can never observe a principal.

use axum::{
    Json,
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;
use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::auth::{actions::validate_access, domain::PrincipalContext};

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

fn json_no_store(status: StatusCode, value: impl Serialize) -> Response {
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(value),
    )
        .into_response()
}

pub(crate) fn unauthorized_bearer() -> Response {
    let mut response = json_no_store(
        StatusCode::UNAUTHORIZED,
        ErrorBody {
            error: "unauthorized",
        },
    );
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Bearer error=\"invalid_token\""),
    );
    response
}

pub(crate) fn service_unavailable() -> Response {
    json_no_store(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorBody {
            error: "unavailable",
        },
    )
}

fn misconfigured() -> Response {
    json_no_store(
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorBody {
            error: "unavailable",
        },
    )
}

/// Exactly one `Authorization: Bearer <credential>` header value. Missing,
/// duplicated, wrong-scheme, query-parameter, and cookie credentials are
/// rejected without inspecting persistence.
pub(crate) fn bearer_credential(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    value.strip_prefix("Bearer ")
}

/// Deny the request before the handler runs unless a live persisted session
/// proves the presented access credential. Database failure denies closed.
pub async fn require_access_session(
    State(database): State<DatabaseConnection>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(presented) = bearer_credential(request.headers()) else {
        return unauthorized_bearer();
    };
    let session = match validate_access::execute(&database, presented).await {
        Ok(session) => session,
        Err(validate_access::ValidateAccessError::Unauthorized) => {
            return unauthorized_bearer();
        }
        Err(validate_access::ValidateAccessError::Persistence(_)) => {
            return service_unavailable();
        }
    };
    // Streams also need the family (per-session limits) and the access expiry.
    request.extensions_mut().insert(session);
    request
        .extensions_mut()
        .insert(PrincipalContext::from(session));
    next.run(request).await
}

impl FromRequestParts<AppContext> for PrincipalContext {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        // The middleware inserts the validated principal. Absence means the
        // route was composed without the middleware, which must never yield
        // an anonymous principal.
        parts
            .extensions
            .get::<PrincipalContext>()
            .copied()
            .ok_or_else(misconfigured)
    }
}

#[cfg(test)]
mod tests {
    use super::bearer_credential;
    use axum::http::{HeaderMap, HeaderValue, header};

    fn headers(values: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn bearer_parsing_requires_exactly_one_bearer_value() {
        assert_eq!(bearer_credential(&headers(&["Bearer abc"])), Some("abc"));
        assert_eq!(bearer_credential(&HeaderMap::new()), None);
        assert_eq!(bearer_credential(&headers(&["Basic abc"])), None);
        assert_eq!(bearer_credential(&headers(&["Bearer "])), Some(""));
        assert_eq!(bearer_credential(&headers(&["Bearer a", "Bearer b"])), None);
        assert_eq!(bearer_credential(&headers(&["bearer abc"])), None);
    }
}
