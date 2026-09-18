use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
use uuid::Uuid;

use crate::auth::github::{self, GitHubAuthService, LOGIN_ATTEMPT_TTL_MINUTES};

pub struct StartedLogin {
    pub attempt_id: Uuid,
    pub authorize_url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StartLoginError {
    #[error("code challenge is not valid")]
    InvalidChallenge,
    #[error("github login is not configured")]
    Disabled,
    #[error("login attempt persistence failed")]
    Persistence,
}

/// Create a ten-minute attempt holding only the state hash and the
/// desktop-supplied S256 challenge. The verifier never reaches the server
/// here; the plaintext state leaves only inside the authorization URL.
pub async fn execute(
    database: &DatabaseConnection,
    service: &GitHubAuthService,
    controller_origin: &str,
    code_challenge: &str,
) -> Result<StartedLogin, StartLoginError> {
    if !service.is_enabled() {
        return Err(StartLoginError::Disabled);
    }
    if !github::valid_challenge(code_challenge) {
        return Err(StartLoginError::InvalidChallenge);
    }
    let redirect_uri =
        github::callback_url(controller_origin).map_err(|_| StartLoginError::Disabled)?;
    let (state, state_hash) = github::generate_oauth_state();
    let now = Utc::now();
    let expires_at = now + Duration::minutes(LOGIN_ATTEMPT_TTL_MINUTES);

    let attempt = crate::auth::entities::github_login_attempt::ActiveModel {
        id: Set(Uuid::now_v7()),
        state_hash: Set(state_hash),
        code_challenge: Set(code_challenge.to_owned()),
        status: Set("pending".to_owned()),
        provider_code: Set(None),
        expires_at: Set(expires_at),
        consumed_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };
    let attempt_id = attempt.id.clone().unwrap();
    attempt
        .insert(database)
        .await
        .map_err(|_| StartLoginError::Persistence)?;

    Ok(StartedLogin {
        attempt_id,
        authorize_url: github::authorize_url(
            &service.config,
            &redirect_uri,
            &state,
            code_challenge,
        ),
        expires_at,
    })
}
