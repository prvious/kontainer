use chrono::Utc;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    sea_query::{Expr, ExprTrait, Func},
};
use uuid::Uuid;

use crate::auth::{
    actions::create_session,
    domain::{IssuedSession, PrincipalStatus},
    entities::{password_credential, principal},
    password,
    throttle::{FailureOutcome, LoginThrottle},
    validation::{FieldErrors, email_key, email_problem, password_problem},
};

#[derive(Debug)]
pub struct PasswordSession {
    pub issued: IssuedSession,
    pub principal_id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PasswordLoginError {
    #[error("sign-in input is invalid")]
    Invalid(FieldErrors),
    /// Unknown email, wrong password, and an inactive owner are indistinguishable.
    #[error("email or password is incorrect")]
    InvalidCredentials { attempts_remaining: u32 },
    #[error("sign-in is locked for this email")]
    Locked { retry_after_seconds: u32 },
    #[error("sign-in persistence failed")]
    Persistence,
}

/// Find a principal by email case-insensitively, with its password credential.
pub(crate) async fn find_by_email(
    database: &DatabaseConnection,
    key: &str,
) -> Result<Option<(principal::Model, Option<password_credential::Model>)>, sea_orm::DbErr> {
    principal::Entity::find()
        .filter(ExprTrait::eq(
            Expr::expr(Func::lower(Expr::col((
                principal::Entity,
                principal::Column::Email,
            )))),
            key,
        ))
        .filter(principal::Column::Email.is_not_null())
        .find_also_related(password_credential::Entity)
        .one(database)
        .await
}

/// Email and password sign-in with per-email throttling. Every failure costs
/// one Argon2id verification, including an unknown email.
pub async fn execute(
    database: &DatabaseConnection,
    throttle: &LoginThrottle,
    email: &str,
    password_input: &str,
) -> Result<PasswordSession, PasswordLoginError> {
    let errors = FieldErrors {
        email: email_problem(email),
        password: password_problem(password_input),
        display_name: None,
    };
    if !errors.is_empty() {
        return Err(PasswordLoginError::Invalid(errors));
    }
    let key = email_key(email);
    let found = find_by_email(database, &key)
        .await
        .map_err(|_| PasswordLoginError::Persistence)?;
    let credential_changed_at = found
        .as_ref()
        .and_then(|(_, credential)| credential.as_ref().map(|credential| credential.updated_at));
    if let Some(retry_after_seconds) = throttle.locked(&key, Utc::now(), credential_changed_at) {
        return Err(PasswordLoginError::Locked {
            retry_after_seconds,
        });
    }

    let stored_hash = found.as_ref().and_then(|(_, credential)| {
        credential
            .as_ref()
            .map(|credential| credential.password_hash.clone())
    });
    let verified = password::verify(password_input.to_owned(), stored_hash).await;
    let principal = found
        .map(|(principal, _)| principal)
        .filter(|principal| verified && principal.status == PrincipalStatus::Active.as_str());
    let Some(principal) = principal else {
        return Err(match throttle.record_failure(&key, Utc::now()) {
            FailureOutcome::Remaining(attempts_remaining) => {
                PasswordLoginError::InvalidCredentials { attempts_remaining }
            }
            FailureOutcome::Locked {
                retry_after_seconds,
            } => PasswordLoginError::Locked {
                retry_after_seconds,
            },
        });
    };
    throttle.clear(&key);

    let issued = create_session::execute(database, principal.id)
        .await
        .map_err(|_| PasswordLoginError::Persistence)?;
    Ok(PasswordSession {
        issued,
        principal_id: principal.id,
        email: principal.email.unwrap_or_default(),
        display_name: principal.display_name.unwrap_or_default(),
    })
}
