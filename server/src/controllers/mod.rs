pub mod account;
pub mod containers;
pub mod github_login;
pub mod health;
pub mod nodes;
pub mod operations;
pub mod password_auth;
pub mod sessions;
pub mod settings;
pub mod status;

use loco_rs::controller::Routes;

/// Authentication-lifecycle routes. These validate their own attempt,
/// refresh, or logout credentials and are mounted outside access-session
/// middleware. Membership here is explicit; never infer it from a path
/// prefix.
pub fn auth_lifecycle_routes() -> Vec<Routes> {
    vec![
        sessions::routes(),
        password_auth::routes(),
        github_login::routes(),
    ]
}

/// Application routes. Every route in this group requires a live access
/// session. Callers must layer [`crate::auth::request::require_access_session`]
/// over each group before nesting; handlers additionally extract
/// [`crate::auth::domain::PrincipalContext`] so a miscomposed route fails
/// closed instead of serving anonymously.
pub fn protected_application_routes() -> Vec<Routes> {
    vec![
        status::routes(),
        account::routes(),
        operations::routes(),
        nodes::routes(),
        settings::routes(),
        containers::routes(),
    ]
}
