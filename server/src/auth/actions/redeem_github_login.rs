use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, QuerySelect,
    TransactionError, TransactionTrait, sea_query::LockType,
};
use uuid::Uuid;

use crate::auth::{
    actions::{claim_bootstrap_owner, create_session},
    domain::{IssuedSession, PrincipalStatus},
    entities::{bootstrap_state, github_identity, github_login_attempt, principal},
    github::{
        self, GitHubAccount, GitHubAuthService, GitHubProviderError, challenge_matches_verifier,
        valid_verifier,
    },
};

#[derive(Debug)]
pub struct RedeemedLogin {
    pub issued: IssuedSession,
    pub principal_id: Uuid,
    pub github_login: String,
    pub bootstrap_claimed: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RedeemError {
    #[error("login attempt is not ready")]
    NotReady,
    #[error("github login is denied")]
    Denied,
    #[error("github service unavailable")]
    ProviderUnavailable,
    #[error("github login is not configured")]
    Disabled,
    #[error("login redemption persistence failed")]
    Persistence,
}

enum TakeOutcome {
    NotReady,
    Denied,
    Ready { code: String },
}

enum FinishOutcome {
    Redeemed(RedeemedLogin),
    Denied,
}

/// Exchange the held provider code for the GitHub identity, then claim or
/// verify the singleton bootstrap owner and issue a session. The provider
/// code is taken and cleared before the network call, so a crash after the
/// take leaves the attempt terminal and requires a fresh login.
pub async fn execute(
    database: &DatabaseConnection,
    service: &GitHubAuthService,
    controller_origin: &str,
    attempt_id: Uuid,
    verifier: &str,
) -> Result<RedeemedLogin, RedeemError> {
    if !service.is_enabled() {
        return Err(RedeemError::Disabled);
    }
    if !valid_verifier(verifier) {
        return Err(RedeemError::Denied);
    }
    let verifier = verifier.to_owned();
    let redirect_uri =
        github::callback_url(controller_origin).map_err(|_| RedeemError::Disabled)?;

    let code = take_provider_code(database, attempt_id, verifier.clone()).await?;
    let account = match service
        .provider
        .resolve_identity(&code, &verifier, &redirect_uri)
        .await
    {
        Ok(account) => account,
        Err(GitHubProviderError::Denied) => {
            consume_attempt(database, attempt_id).await?;
            return Err(RedeemError::Denied);
        }
        Err(GitHubProviderError::Unavailable) => {
            consume_attempt(database, attempt_id).await?;
            return Err(RedeemError::ProviderUnavailable);
        }
    };
    if !service.config.allowlisted.contains(&account.account_id) {
        consume_attempt(database, attempt_id).await?;
        return Err(RedeemError::Denied);
    }

    match finish_login(database, attempt_id, account.clone()).await? {
        FinishOutcome::Redeemed(redeemed) => Ok(redeemed),
        FinishOutcome::Denied => Err(RedeemError::Denied),
    }
}

async fn take_provider_code(
    database: &DatabaseConnection,
    attempt_id: Uuid,
    verifier: String,
) -> Result<String, RedeemError> {
    let outcome = database
        .transaction(|transaction| {
            Box::pin(async move {
                let attempt = github_login_attempt::Entity::find_by_id(attempt_id)
                    .lock(LockType::Update)
                    .one(transaction)
                    .await
                    .map_err(|_| RedeemError::Persistence)?;
                let Some(attempt) = attempt else {
                    return Ok(TakeOutcome::Denied);
                };
                if attempt.status == "consumed" {
                    return Ok(TakeOutcome::Denied);
                }
                let now = Utc::now();
                if attempt.expires_at <= now {
                    consume_in(transaction, &attempt, now).await?;
                    return Ok(TakeOutcome::Denied);
                }
                if attempt.status == "pending" {
                    // The browser has not completed yet. Report pending
                    // without consuming so the desktop can retry.
                    return Ok(TakeOutcome::NotReady);
                }
                if attempt.status != "ready" {
                    return Ok(TakeOutcome::Denied);
                }
                if !challenge_matches_verifier(&attempt.code_challenge, &verifier) {
                    // A wrong verifier does not consume the attempt: only
                    // the desktop holding the original verifier can retry.
                    return Ok(TakeOutcome::Denied);
                }
                let Some(code) = attempt.provider_code.clone() else {
                    consume_in(transaction, &attempt, now).await?;
                    return Ok(TakeOutcome::Denied);
                };
                let mut redeeming: github_login_attempt::ActiveModel = attempt.into();
                redeeming.status = Set("redeeming".to_owned());
                redeeming.provider_code = Set(None);
                redeeming.updated_at = Set(now);
                redeeming
                    .update(transaction)
                    .await
                    .map_err(|_| RedeemError::Persistence)?;
                Ok(TakeOutcome::Ready { code })
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(_) => RedeemError::Persistence,
            TransactionError::Transaction(error) => error,
        })?;

    match outcome {
        TakeOutcome::Ready { code } => Ok(code),
        TakeOutcome::NotReady => Err(RedeemError::NotReady),
        TakeOutcome::Denied => Err(RedeemError::Denied),
    }
}

async fn consume_in<C>(
    database: &C,
    attempt: &github_login_attempt::Model,
    now: chrono::DateTime<Utc>,
) -> Result<(), RedeemError>
where
    C: sea_orm::ConnectionTrait,
{
    let mut consumed: github_login_attempt::ActiveModel = attempt.clone().into();
    consumed.status = Set("consumed".to_owned());
    consumed.provider_code = Set(None);
    consumed.consumed_at = Set(Some(now));
    consumed.updated_at = Set(now);
    consumed
        .update(database)
        .await
        .map_err(|_| RedeemError::Persistence)?;
    Ok(())
}

async fn consume_attempt(
    database: &DatabaseConnection,
    attempt_id: Uuid,
) -> Result<(), RedeemError> {
    database
        .transaction(|transaction| {
            Box::pin(async move {
                let attempt = github_login_attempt::Entity::find_by_id(attempt_id)
                    .lock(LockType::Update)
                    .one(transaction)
                    .await
                    .map_err(|_| RedeemError::Persistence)?;
                let Some(attempt) = attempt else {
                    return Ok(());
                };
                if attempt.status == "consumed" {
                    return Ok(());
                }
                consume_in(transaction, &attempt, Utc::now()).await
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(_) => RedeemError::Persistence,
            TransactionError::Transaction(error) => error,
        })
}

async fn finish_login(
    database: &DatabaseConnection,
    attempt_id: Uuid,
    account: GitHubAccount,
) -> Result<FinishOutcome, RedeemError> {
    database
        .transaction(|transaction| {
            Box::pin(async move {
                let attempt = github_login_attempt::Entity::find_by_id(attempt_id)
                    .lock(LockType::Update)
                    .one(transaction)
                    .await
                    .map_err(|_| RedeemError::Persistence)?;
                let Some(attempt) = attempt else {
                    return Ok(FinishOutcome::Denied);
                };
                if attempt.status != "redeeming" {
                    return Ok(FinishOutcome::Denied);
                }
                // The singleton row serializes concurrent first logins:
                // exactly one transaction observes the unclaimed state.
                let bootstrap = claim_bootstrap_owner::lock_singleton(transaction)
                    .await
                    .map_err(|_| RedeemError::Persistence)?;
                let Some(bootstrap) = bootstrap else {
                    return Ok(FinishOutcome::Denied);
                };
                let now = Utc::now();
                match bootstrap.owner_principal_id {
                    None => {
                        claim_bootstrap_owner(transaction, &attempt, &bootstrap, &account, now)
                            .await
                    }
                    Some(owner) => {
                        verify_established_owner(transaction, &attempt, owner, &account, now).await
                    }
                }
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(_) => RedeemError::Persistence,
            TransactionError::Transaction(error) => error,
        })
}

/// First successful allowlisted login: create the principal, bind the
/// GitHub identity, claim the singleton, and issue the session atomically.
async fn claim_bootstrap_owner<C>(
    database: &C,
    attempt: &github_login_attempt::Model,
    bootstrap: &bootstrap_state::Model,
    account: &GitHubAccount,
    now: chrono::DateTime<Utc>,
) -> Result<FinishOutcome, RedeemError>
where
    C: sea_orm::ConnectionTrait,
{
    let principal_id = claim_bootstrap_owner::insert_owner_principal(database, now, None, None)
        .await
        .map_err(|_| RedeemError::Persistence)?;

    github_identity::ActiveModel {
        github_account_id: Set(account.account_id),
        principal_id: Set(principal_id),
        login: Set(account.login.clone()),
        created_at: Set(now),
        updated_at: Set(now),
        last_authenticated_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(|_| RedeemError::Persistence)?;

    claim_bootstrap_owner::claim(database, bootstrap, principal_id, now)
        .await
        .map_err(|_| RedeemError::Persistence)?;

    let issued = create_session::create_in_transaction(database, principal_id)
        .await
        .map_err(|_| RedeemError::Persistence)?;
    consume_in(database, attempt, now).await?;

    Ok(FinishOutcome::Redeemed(RedeemedLogin {
        issued,
        principal_id,
        github_login: account.login.clone(),
        bootstrap_claimed: true,
    }))
}

/// Later logins succeed only for the persisted bootstrap identity. Any
/// other account, dangling identity, or inactive principal is denied
/// without creating rows.
async fn verify_established_owner<C>(
    database: &C,
    attempt: &github_login_attempt::Model,
    owner: Uuid,
    account: &GitHubAccount,
    now: chrono::DateTime<Utc>,
) -> Result<FinishOutcome, RedeemError>
where
    C: sea_orm::ConnectionTrait,
{
    let identity = github_identity::Entity::find_by_id(account.account_id)
        .one(database)
        .await
        .map_err(|_| RedeemError::Persistence)?;
    let Some(identity) = identity else {
        consume_in(database, attempt, now).await?;
        return Ok(FinishOutcome::Denied);
    };
    if identity.principal_id != owner {
        consume_in(database, attempt, now).await?;
        return Ok(FinishOutcome::Denied);
    }
    let principal = principal::Entity::find_by_id(owner)
        .one(database)
        .await
        .map_err(|_| RedeemError::Persistence)?;
    let Some(principal) = principal else {
        consume_in(database, attempt, now).await?;
        return Ok(FinishOutcome::Denied);
    };
    if principal.status != PrincipalStatus::Active.as_str() {
        consume_in(database, attempt, now).await?;
        return Ok(FinishOutcome::Denied);
    }

    let mut identity_model: github_identity::ActiveModel = identity.into();
    identity_model.login = Set(account.login.clone());
    identity_model.updated_at = Set(now);
    identity_model.last_authenticated_at = Set(now);
    identity_model
        .update(database)
        .await
        .map_err(|_| RedeemError::Persistence)?;

    let issued = create_session::create_in_transaction(database, owner)
        .await
        .map_err(|_| RedeemError::Persistence)?;
    consume_in(database, attempt, now).await?;

    Ok(FinishOutcome::Redeemed(RedeemedLogin {
        issued,
        principal_id: owner,
        github_login: account.login.clone(),
        bootstrap_claimed: false,
    }))
}
