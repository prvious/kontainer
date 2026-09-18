use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, TransactionError, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::operations::{
    domain::{AdmitOperationRequest, CorruptOperationState, DurableOperation, OperationState},
    entities::{audit_intent, idempotency_record, operation, operation_event},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitOperationOutcome {
    pub operation: DurableOperation,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum AdmitOperationError {
    #[error("the idempotency key was already used for a different request")]
    IdempotencyConflict { operation_id: Uuid },
    #[error("an idempotency record refers to a missing operation")]
    MissingOperation,
    #[error(transparent)]
    CorruptState(#[from] CorruptOperationState),
    #[error("operation admission persistence failed")]
    Persistence(#[source] DbErr),
    #[cfg(test)]
    #[error("injected admission failure")]
    InjectedFailure,
}

#[derive(Clone)]
struct AdmissionMaterial {
    principal_ref: String,
    scope: String,
    key_hash: String,
    request_hash: String,
    kind: String,
    audit_action: String,
    audit_target_ref: Option<String>,
    expires_at: chrono::DateTime<Utc>,
}

enum AdmissionAttempt {
    Created(operation::Model),
    Existing(idempotency_record::Model),
}

#[derive(Debug, thiserror::Error)]
enum AdmissionAttemptError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[cfg(test)]
    #[error("injected admission failure")]
    InjectedFailure,
}

#[derive(Clone, Copy)]
enum FailurePoint {
    None,
    #[cfg(test)]
    AfterInitialEvent,
}

pub async fn execute(
    database: &DatabaseConnection,
    request: AdmitOperationRequest,
) -> Result<AdmitOperationOutcome, AdmitOperationError> {
    execute_at_failure_point(database, request, FailurePoint::None).await
}

async fn execute_at_failure_point(
    database: &DatabaseConnection,
    request: AdmitOperationRequest,
    failure_point: FailurePoint,
) -> Result<AdmitOperationOutcome, AdmitOperationError> {
    let material = AdmissionMaterial {
        principal_ref: request.principal().reference(),
        scope: request.scope().to_owned(),
        key_hash: sha256(request.idempotency_key().as_bytes()),
        request_hash: sha256(request.canonical_request()),
        kind: request.kind().to_owned(),
        audit_action: request.audit_action().to_owned(),
        audit_target_ref: request.audit_target_ref().map(ToOwned::to_owned),
        expires_at: request.idempotency_expires_at(),
    };
    let transaction_material = material.clone();

    let attempt = database
        .transaction(move |transaction| {
            Box::pin(async move {
                admit_in_transaction(transaction, &transaction_material, failure_point).await
            })
        })
        .await;

    match attempt {
        Ok(AdmissionAttempt::Created(model)) => Ok(AdmitOperationOutcome {
            operation: model.try_into()?,
            replayed: false,
        }),
        Ok(AdmissionAttempt::Existing(record)) => {
            resolve_existing(database, record, &material.request_hash).await
        }
        Err(TransactionError::Connection(error)) => Err(AdmitOperationError::Persistence(error)),
        Err(TransactionError::Transaction(AdmissionAttemptError::Database(error))) => {
            // A concurrent request can win the unique idempotency key race after
            // this transaction's initial lookup. The failed transaction has
            // already rolled back every inserted row before we resolve the winner.
            match find_idempotency_record(database, &material)
                .await
                .map_err(AdmitOperationError::Persistence)?
            {
                Some(record) => resolve_existing(database, record, &material.request_hash).await,
                None => Err(AdmitOperationError::Persistence(error)),
            }
        }
        #[cfg(test)]
        Err(TransactionError::Transaction(AdmissionAttemptError::InjectedFailure)) => {
            Err(AdmitOperationError::InjectedFailure)
        }
    }
}

async fn admit_in_transaction<C>(
    database: &C,
    material: &AdmissionMaterial,
    failure_point: FailurePoint,
) -> Result<AdmissionAttempt, AdmissionAttemptError>
where
    C: ConnectionTrait,
{
    if let Some(record) = find_idempotency_record(database, material).await? {
        return Ok(AdmissionAttempt::Existing(record));
    }

    let now = Utc::now();
    let operation_id = Uuid::now_v7();
    let created_operation = operation::ActiveModel {
        id: Set(operation_id),
        kind: Set(material.kind.clone()),
        state: Set(OperationState::Recoverable.as_str().to_owned()),
        claimed_by: Set(None),
        claimed_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await?;

    idempotency_record::ActiveModel {
        id: Set(Uuid::now_v7()),
        principal_ref: Set(material.principal_ref.clone()),
        scope: Set(material.scope.clone()),
        key_hash: Set(material.key_hash.clone()),
        request_hash: Set(material.request_hash.clone()),
        operation_id: Set(operation_id),
        expires_at: Set(material.expires_at),
        created_at: Set(now),
    }
    .insert(database)
    .await?;

    operation_event::ActiveModel {
        operation_id: Set(operation_id),
        sequence: Set(1),
        state: Set(OperationState::Recoverable.as_str().to_owned()),
        error_code: Set(None),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(database)
    .await?;

    #[cfg(test)]
    if matches!(failure_point, FailurePoint::AfterInitialEvent) {
        return Err(AdmissionAttemptError::InjectedFailure);
    }
    #[cfg(not(test))]
    let _ = failure_point;

    audit_intent::ActiveModel {
        id: Set(Uuid::now_v7()),
        operation_id: Set(operation_id),
        principal_ref: Set(material.principal_ref.clone()),
        action: Set(material.audit_action.clone()),
        target_ref: Set(material.audit_target_ref.clone()),
        state: Set("pending".to_owned()),
        created_at: Set(now),
    }
    .insert(database)
    .await?;

    Ok(AdmissionAttempt::Created(created_operation))
}

async fn find_idempotency_record<C>(
    database: &C,
    material: &AdmissionMaterial,
) -> Result<Option<idempotency_record::Model>, DbErr>
where
    C: ConnectionTrait,
{
    idempotency_record::Entity::find()
        .filter(idempotency_record::Column::PrincipalRef.eq(&material.principal_ref))
        .filter(idempotency_record::Column::Scope.eq(&material.scope))
        .filter(idempotency_record::Column::KeyHash.eq(&material.key_hash))
        .one(database)
        .await
}

async fn resolve_existing(
    database: &DatabaseConnection,
    record: idempotency_record::Model,
    request_hash: &str,
) -> Result<AdmitOperationOutcome, AdmitOperationError> {
    if record.request_hash != request_hash {
        return Err(AdmitOperationError::IdempotencyConflict {
            operation_id: record.operation_id,
        });
    }

    let operation = operation::Entity::find_by_id(record.operation_id)
        .one(database)
        .await
        .map_err(AdmitOperationError::Persistence)?
        .ok_or(AdmitOperationError::MissingOperation)?
        .try_into()?;

    Ok(AdmitOperationOutcome {
        operation,
        replayed: true,
    })
}

fn sha256(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

#[cfg(test)]
pub(crate) async fn execute_with_injected_rollback(
    database: &DatabaseConnection,
    request: AdmitOperationRequest,
) -> Result<AdmitOperationOutcome, AdmitOperationError> {
    execute_at_failure_point(database, request, FailurePoint::AfterInitialEvent).await
}
