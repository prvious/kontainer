use sea_orm_migration::{prelude::*, schema::*};

/// Executable operations (ADR 0009): the admitted payload, execution times, and the admitting
/// request id on `operations`; progress detail and failure messages on `operation_events`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Operation::Table)
                    .add_column(json_binary(Operation::Payload).default(Expr::cust("'{}'::jsonb")))
                    .add_column(timestamp_with_time_zone_null(Operation::StartedAt))
                    .add_column(timestamp_with_time_zone_null(Operation::FinishedAt))
                    .add_column(string_len_null(Operation::RequestId, 128))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(OperationEvent::Table)
                    .add_column(json_binary_null(OperationEvent::Detail))
                    .add_column(text_null(OperationEvent::Message))
                    .to_owned(),
            )
            .await?;
        // Pruning scans terminal operations by age.
        manager
            .create_index(
                Index::create()
                    .name("idx_operations_state_updated_at")
                    .table(Operation::Table)
                    .col(Operation::State)
                    .col(Operation::UpdatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_operations_state_updated_at")
                    .table(Operation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(OperationEvent::Table)
                    .drop_column(OperationEvent::Message)
                    .drop_column(OperationEvent::Detail)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Operation::Table)
                    .drop_column(Operation::RequestId)
                    .drop_column(Operation::FinishedAt)
                    .drop_column(Operation::StartedAt)
                    .drop_column(Operation::Payload)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Operation {
    #[sea_orm(iden = "operations")]
    Table,
    State,
    UpdatedAt,
    Payload,
    StartedAt,
    FinishedAt,
    RequestId,
}

#[derive(DeriveIden)]
enum OperationEvent {
    #[sea_orm(iden = "operation_events")]
    Table,
    Detail,
    Message,
}
