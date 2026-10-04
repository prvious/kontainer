use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionError, TransactionTrait,
};

use crate::auth::{
    actions::{password_login::find_by_email, revoke_session_family},
    domain::RevocationReason,
    entities::{password_credential, session_family},
    password,
    validation::{FieldProblem, email_key, password_problem},
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResetPasswordError {
    #[error("the new password is not acceptable: {0:?}")]
    InvalidPassword(FieldProblem),
    #[error("no account uses that email")]
    UnknownEmail,
    #[error("the controller is busy checking passwords; try again")]
    Busy,
    #[error("password reset persistence failed")]
    Persistence,
}

/// Operator recovery from the controller host (`auth:reset-password`):
/// rehash, then revoke every session family of the principal. The new
/// credential timestamp also voids any active sign-in lock for the email.
/// Returns how many live sessions were revoked.
pub async fn execute(
    database: &DatabaseConnection,
    email: &str,
    new_password: String,
) -> Result<usize, ResetPasswordError> {
    if let Some(problem) = password_problem(&new_password) {
        return Err(ResetPasswordError::InvalidPassword(problem));
    }
    let (principal, credential) = find_by_email(database, &email_key(email))
        .await
        .map_err(|_| ResetPasswordError::Persistence)?
        .ok_or(ResetPasswordError::UnknownEmail)?;
    let permit = password::permit()
        .await
        .map_err(|_| ResetPasswordError::Busy)?;
    let password_hash = password::hash(permit, new_password)
        .await
        .map_err(|_| ResetPasswordError::Persistence)?;

    database
        .transaction(|transaction| {
            Box::pin(async move {
                let now = Utc::now();
                match credential {
                    Some(credential) => {
                        let mut updated: password_credential::ActiveModel = credential.into();
                        updated.password_hash = Set(password_hash);
                        updated.updated_at = Set(now);
                        updated
                            .update(transaction)
                            .await
                            .map_err(|_| ResetPasswordError::Persistence)?;
                    }
                    None => {
                        password_credential::ActiveModel {
                            principal_id: Set(principal.id),
                            password_hash: Set(password_hash),
                            created_at: Set(now),
                            updated_at: Set(now),
                        }
                        .insert(transaction)
                        .await
                        .map_err(|_| ResetPasswordError::Persistence)?;
                    }
                }
                let live = session_family::Entity::find()
                    .filter(session_family::Column::PrincipalId.eq(principal.id))
                    .filter(session_family::Column::RevokedAt.is_null())
                    .all(transaction)
                    .await
                    .map_err(|_| ResetPasswordError::Persistence)?;
                for family in &live {
                    revoke_session_family::revoke_in_transaction(
                        transaction,
                        family.id,
                        RevocationReason::PasswordReset,
                    )
                    .await
                    .map_err(|_| ResetPasswordError::Persistence)?;
                }
                Ok(live.len())
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(_) => ResetPasswordError::Persistence,
            TransactionError::Transaction(error) => error,
        })
}
