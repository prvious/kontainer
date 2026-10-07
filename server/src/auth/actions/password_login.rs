use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, TransactionError,
    TransactionTrait,
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
    /// Every Argon2id permit is in use; not counted against the email.
    #[error("too many sign-ins are being checked")]
    Busy,
    #[error("sign-in persistence failed")]
    Persistence,
}

const fn failed(outcome: FailureOutcome) -> PasswordLoginError {
    match outcome {
        FailureOutcome::Remaining(attempts_remaining) => {
            PasswordLoginError::InvalidCredentials { attempts_remaining }
        }
        FailureOutcome::Locked {
            retry_after_seconds,
        } => PasswordLoginError::Locked {
            retry_after_seconds,
        },
    }
}

/// Issue the session only if the password credential is still the one that was verified. The
/// shared row lock orders this against a reset: a reset that commits first changes `updated_at`
/// and the sign-in fails; one that commits after sees and revokes the new session.
pub(crate) async fn create_if_credential_unchanged(
    database: &DatabaseConnection,
    principal_id: Uuid,
    verified_at: DateTime<Utc>,
) -> Result<Option<IssuedSession>, PasswordLoginError> {
    database
        .transaction(|transaction| {
            Box::pin(async move {
                let current = password_credential::Entity::find_by_id(principal_id)
                    .lock_shared()
                    .one(transaction)
                    .await
                    .map_err(|_| PasswordLoginError::Persistence)?;
                if current.map(|credential| credential.updated_at) != Some(verified_at) {
                    return Ok(None);
                }
                create_session::create_in_transaction(transaction, principal_id)
                    .await
                    .map(Some)
                    .map_err(|_| PasswordLoginError::Persistence)
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(_) => PasswordLoginError::Persistence,
            TransactionError::Transaction(error) => error,
        })
}

/// Find a principal by email case-insensitively, with its password credential. PostgreSQL
/// lowers both sides, the same way the unique index does: Rust and PostgreSQL disagree on some
/// non-ASCII characters, and a mixed comparison could lock the owner out.
pub(crate) async fn find_by_email(
    database: &DatabaseConnection,
    email: &str,
) -> Result<Option<(principal::Model, Option<password_credential::Model>)>, sea_orm::DbErr> {
    principal::Entity::find()
        .filter(ExprTrait::eq(
            Expr::expr(Func::lower(Expr::col((
                principal::Entity,
                principal::Column::Email,
            )))),
            Func::lower(Expr::val(email.trim())),
        ))
        .filter(principal::Column::Email.is_not_null())
        .find_also_related(password_credential::Entity)
        .one(database)
        .await
}

/// Email and password sign-in with per-email throttling. Every attempt costs
/// one Argon2id verification, including an unknown email, and is counted before
/// that verification runs.
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
        setup_code: None,
    };
    if !errors.is_empty() {
        return Err(PasswordLoginError::Invalid(errors));
    }
    let key = email_key(email);
    let found = find_by_email(database, email)
        .await
        .map_err(|_| PasswordLoginError::Persistence)?;
    let credential = found
        .as_ref()
        .and_then(|(_, credential)| credential.as_ref());
    let credential_changed_at = credential.map(|credential| credential.updated_at);
    let stored_hash = credential.map(|credential| credential.password_hash.clone());

    // The permit comes first so a busy refusal is never counted against the email.
    let permit = password::permit()
        .await
        .map_err(|_| PasswordLoginError::Busy)?;
    let if_failed = throttle
        .begin_attempt(&key, Utc::now(), credential_changed_at)
        .map_err(|retry_after_seconds| PasswordLoginError::Locked {
            retry_after_seconds,
        })?;
    let verified = password::verify(permit, password_input.to_owned(), stored_hash).await;
    let principal = found
        .map(|(principal, _)| principal)
        .filter(|principal| verified && principal.status == PrincipalStatus::Active.as_str());
    let (Some(principal), Some(verified_at)) = (principal, credential_changed_at) else {
        return Err(failed(if_failed));
    };

    let Some(issued) = create_if_credential_unchanged(database, principal.id, verified_at)
        .await
        .inspect_err(|_| throttle.clear(&key))?
    else {
        // The password was reset while this one was being verified.
        return Err(failed(if_failed));
    };
    throttle.clear(&key);
    Ok(PasswordSession {
        issued,
        principal_id: principal.id,
        email: principal.email.unwrap_or_default(),
        display_name: principal.display_name.unwrap_or_default(),
    })
}
