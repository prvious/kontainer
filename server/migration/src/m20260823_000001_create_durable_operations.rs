use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Operation::Table)
                    .if_not_exists()
                    .col(pk_uuid(Operation::Id))
                    .col(string_len(Operation::Kind, 64))
                    .col(
                        ColumnDef::new(Operation::State)
                            .string_len(32)
                            .not_null()
                            .check((
                                "operations_state_valid",
                                Expr::col(Operation::State).is_in([
                                    "recoverable",
                                    "running",
                                    "uncertain",
                                    "succeeded",
                                    "failed",
                                    "cancelled",
                                ]),
                            )),
                    )
                    .col(string_len_null(Operation::ClaimedBy, 128))
                    .col(timestamp_with_time_zone_null(Operation::ClaimedAt))
                    .col(timestamp_with_time_zone_default_now(Operation::CreatedAt))
                    .col(timestamp_with_time_zone_default_now(Operation::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(IdempotencyRecord::Table)
                    .if_not_exists()
                    .col(pk_uuid(IdempotencyRecord::Id))
                    .col(string_len(IdempotencyRecord::PrincipalRef, 255))
                    .col(string_len(IdempotencyRecord::Scope, 128))
                    .col(char_len(IdempotencyRecord::KeyHash, 64))
                    .col(char_len(IdempotencyRecord::RequestHash, 64))
                    .col(uuid(IdempotencyRecord::OperationId))
                    .col(timestamp_with_time_zone(IdempotencyRecord::ExpiresAt))
                    .col(timestamp_with_time_zone_default_now(
                        IdempotencyRecord::CreatedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_idempotency_operation")
                            .from(IdempotencyRecord::Table, IdempotencyRecord::OperationId)
                            .to(Operation::Table, Operation::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_idempotency_principal_scope_key")
                    .table(IdempotencyRecord::Table)
                    .col(IdempotencyRecord::PrincipalRef)
                    .col(IdempotencyRecord::Scope)
                    .col(IdempotencyRecord::KeyHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OperationEvent::Table)
                    .if_not_exists()
                    .col(big_pk_auto(OperationEvent::Id))
                    .col(uuid(OperationEvent::OperationId))
                    .col(integer(OperationEvent::Sequence))
                    .col(string_len(OperationEvent::State, 32))
                    .col(string_len_null(OperationEvent::ErrorCode, 64))
                    .col(timestamp_with_time_zone_default_now(
                        OperationEvent::CreatedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_operation_events_operation")
                            .from(OperationEvent::Table, OperationEvent::OperationId)
                            .to(Operation::Table, Operation::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_operation_events_sequence")
                    .table(OperationEvent::Table)
                    .col(OperationEvent::OperationId)
                    .col(OperationEvent::Sequence)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AuditIntent::Table)
                    .if_not_exists()
                    .col(pk_uuid(AuditIntent::Id))
                    .col(uuid(AuditIntent::OperationId))
                    .col(string_len(AuditIntent::PrincipalRef, 255))
                    .col(string_len(AuditIntent::Action, 64))
                    .col(string_len_null(AuditIntent::TargetRef, 255))
                    .col(string_len(AuditIntent::State, 32))
                    .col(timestamp_with_time_zone_default_now(AuditIntent::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_audit_intents_operation")
                            .from(AuditIntent::Table, AuditIntent::OperationId)
                            .to(Operation::Table, Operation::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_audit_intents_operation")
                    .table(AuditIntent::Table)
                    .col(AuditIntent::OperationId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_operations_claimable")
                    .table(Operation::Table)
                    .col(Operation::State)
                    .col(Operation::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditIntent::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OperationEvent::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(IdempotencyRecord::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Operation::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Operation {
    #[sea_orm(iden = "operations")]
    Table,
    Id,
    Kind,
    State,
    ClaimedBy,
    ClaimedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum IdempotencyRecord {
    #[sea_orm(iden = "idempotency_records")]
    Table,
    Id,
    PrincipalRef,
    Scope,
    KeyHash,
    RequestHash,
    OperationId,
    ExpiresAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum OperationEvent {
    #[sea_orm(iden = "operation_events")]
    Table,
    Id,
    OperationId,
    Sequence,
    State,
    ErrorCode,
    CreatedAt,
}

#[derive(DeriveIden)]
enum AuditIntent {
    #[sea_orm(iden = "audit_intents")]
    Table,
    Id,
    OperationId,
    PrincipalRef,
    Action,
    TargetRef,
    State,
    CreatedAt,
}
