use sea_orm_migration::{prelude::*, schema::*};

/// Node membership and resource samples (ADR 0003): one row per agent that has checked in, its
/// samples at check-in cadence, and the single controller settings row that bounds retention.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Node::Table)
                    .if_not_exists()
                    .col(text(Node::Id).primary_key())
                    .col(text(Node::ClusterId))
                    .col(text(Node::Hostname))
                    .col(text_null(Node::SwarmNodeId))
                    .col(text(Node::AdvertiseAddress))
                    .col(text(Node::AgentVersion))
                    .col(text(Node::DockerVersion))
                    .col(text(Node::Os))
                    .col(text(Node::Arch))
                    .col(integer(Node::CpuCores))
                    .col(big_integer(Node::MemoryTotalBytes))
                    .col(timestamp_with_time_zone(Node::AgentStartedAt))
                    .col(timestamp_with_time_zone(Node::FirstSeenAt))
                    .col(timestamp_with_time_zone(Node::LastCheckInAt))
                    .col(timestamp_with_time_zone_default_now(Node::CreatedAt))
                    .col(timestamp_with_time_zone_default_now(Node::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NodeSample::Table)
                    .if_not_exists()
                    .col(text(NodeSample::NodeId))
                    .col(timestamp_with_time_zone(NodeSample::SampledAt))
                    .col(float(NodeSample::CpuPercent))
                    .col(big_integer(NodeSample::MemoryUsedBytes))
                    .col(big_integer_null(NodeSample::DiskUsedBytes))
                    .col(integer(NodeSample::ContainersRunning))
                    .col(integer(NodeSample::ContainersPaused))
                    .col(integer(NodeSample::ContainersExited))
                    .col(double_null(NodeSample::NetRxBytesPerSec))
                    .col(double_null(NodeSample::NetTxBytesPerSec))
                    .primary_key(
                        Index::create()
                            .col(NodeSample::NodeId)
                            .col(NodeSample::SampledAt),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_node_samples_node")
                            .from(NodeSample::Table, NodeSample::NodeId)
                            .to(Node::Table, Node::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_samples_sampled_at")
                    .table(NodeSample::Table)
                    .col(NodeSample::SampledAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ControllerSetting::Table)
                    .if_not_exists()
                    .col(small_integer(ControllerSetting::Id).primary_key())
                    .col(integer(ControllerSetting::MetricsRetentionDays).default(7))
                    .col(timestamp_with_time_zone_default_now(
                        ControllerSetting::UpdatedAt,
                    ))
                    .check(Expr::col(ControllerSetting::Id).eq(1))
                    .check(Expr::col(ControllerSetting::MetricsRetentionDays).between(1, 365))
                    .to_owned(),
            )
            .await?;

        manager
            .exec_stmt(
                Query::insert()
                    .into_table(ControllerSetting::Table)
                    .columns([ControllerSetting::Id])
                    .values_panic([1.into()])
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ControllerSetting::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NodeSample::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Node::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Node {
    #[sea_orm(iden = "nodes")]
    Table,
    Id,
    ClusterId,
    Hostname,
    SwarmNodeId,
    AdvertiseAddress,
    AgentVersion,
    DockerVersion,
    Os,
    Arch,
    CpuCores,
    MemoryTotalBytes,
    AgentStartedAt,
    FirstSeenAt,
    LastCheckInAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum NodeSample {
    #[sea_orm(iden = "node_samples")]
    Table,
    NodeId,
    SampledAt,
    CpuPercent,
    MemoryUsedBytes,
    DiskUsedBytes,
    ContainersRunning,
    ContainersPaused,
    ContainersExited,
    NetRxBytesPerSec,
    NetTxBytesPerSec,
}

#[derive(DeriveIden)]
enum ControllerSetting {
    #[sea_orm(iden = "controller_settings")]
    Table,
    Id,
    MetricsRetentionDays,
    UpdatedAt,
}
