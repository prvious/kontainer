use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, TransactionError, TransactionTrait,
};
use uuid::Uuid;

use crate::auth::{
    actions::{bootstrap_status, claim_bootstrap_owner, create_session},
    credentials::SetupCode,
    domain::IssuedSession,
    entities::password_credential,
    password,
    validation::{
        FieldErrors, FieldProblem, display_name_problem, email_problem, password_problem,
        setup_code_problem,
    },
};

pub struct OwnerRegistration {
    pub display_name: String,
    pub email: String,
    pub password: String,
    pub setup_code: String,
}

impl std::fmt::Debug for OwnerRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OwnerRegistration")
            .field("password", &"<redacted>")
            .field("setup_code", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct RegisteredOwner {
    pub issued: IssuedSession,
    pub principal_id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RegisterOwnerError {
    #[error("registration input is invalid")]
    Invalid(FieldErrors),
    #[error("the owner already exists")]
    OwnerExists,
    /// Every Argon2id permit is in use.
    #[error("too many password checks are running")]
    Busy,
    #[error("registration persistence failed")]
    Persistence,
}

/// First-run registration: create the owner with an Argon2id password, claim
/// the bootstrap singleton, and issue a session, all in one transaction.
/// Registration needs the controller's setup code and closes for good once an
/// owner exists. Nothing reaches Argon2id until both of those checks pass.
pub async fn execute(
    database: &DatabaseConnection,
    registration: OwnerRegistration,
    expected_code: Option<&SetupCode>,
) -> Result<RegisteredOwner, RegisterOwnerError> {
    let errors = FieldErrors {
        email: email_problem(&registration.email),
        password: password_problem(&registration.password),
        display_name: display_name_problem(&registration.display_name),
        setup_code: setup_code_problem(&registration.setup_code),
    };
    if !errors.is_empty() {
        return Err(RegisterOwnerError::Invalid(errors));
    }
    if bootstrap_status::execute(database)
        .await
        .map_err(|_| RegisterOwnerError::Persistence)?
    {
        return Err(RegisterOwnerError::OwnerExists);
    }
    if !expected_code.is_some_and(|code| code.matches(&registration.setup_code)) {
        return Err(RegisterOwnerError::Invalid(FieldErrors {
            setup_code: Some(FieldProblem::Invalid),
            ..FieldErrors::default()
        }));
    }
    let email = registration.email.trim().to_owned();
    let display_name = registration.display_name.trim().to_owned();
    // Hash before the transaction so the singleton lock is never held across Argon2id.
    let permit = password::permit()
        .await
        .map_err(|_| RegisterOwnerError::Busy)?;
    let password_hash = password::hash(permit, registration.password)
        .await
        .map_err(|_| RegisterOwnerError::Persistence)?;

    database
        .transaction(|transaction| {
            Box::pin(async move {
                let bootstrap = claim_bootstrap_owner::lock_singleton(transaction)
                    .await
                    .map_err(|_| RegisterOwnerError::Persistence)?
                    .ok_or(RegisterOwnerError::Persistence)?;
                if bootstrap.owner_principal_id.is_some() {
                    return Err(RegisterOwnerError::OwnerExists);
                }
                let now = Utc::now();
                let principal_id = claim_bootstrap_owner::insert_owner_principal(
                    transaction,
                    now,
                    Some(email.clone()),
                    Some(display_name.clone()),
                )
                .await
                .map_err(|_| RegisterOwnerError::Persistence)?;
                password_credential::ActiveModel {
                    principal_id: Set(principal_id),
                    password_hash: Set(password_hash),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(transaction)
                .await
                .map_err(|_| RegisterOwnerError::Persistence)?;
                claim_bootstrap_owner::claim(transaction, &bootstrap, principal_id, now)
                    .await
                    .map_err(|_| RegisterOwnerError::Persistence)?;
                let issued = create_session::create_in_transaction(transaction, principal_id)
                    .await
                    .map_err(|_| RegisterOwnerError::Persistence)?;
                Ok(RegisteredOwner {
                    issued,
                    principal_id,
                    email,
                    display_name,
                })
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(_) => RegisterOwnerError::Persistence,
            TransactionError::Transaction(error) => error,
        })
}
