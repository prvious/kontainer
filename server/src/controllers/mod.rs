pub mod health;
pub mod operations;
pub mod sessions;
pub mod status;

use loco_rs::controller::Routes;

/// Authentication-lifecycle routes. These validate their own attempt,
/// refresh, or logout credentials and are mounted outside access-session
/// middleware. Membership here is explicit; never infer it from a path
/// prefix. Future GitHub start/callback/redemption routes from #79 register
/// in this group.
pub fn auth_lifecycle_routes() -> Vec<Routes> {
    vec![sessions::routes()]
}

/// Application routes. Every route in this group requires a live access
/// session. Callers must layer [`crate::auth::request::require_access_session`]
/// over each group before nesting; handlers additionally extract
/// [`crate::auth::domain::PrincipalContext`] so a miscomposed route fails
/// closed instead of serving anonymously.
pub fn protected_application_routes() -> Vec<Routes> {
    vec![status::routes(), operations::routes()]
}
