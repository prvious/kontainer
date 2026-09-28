use chrono::Duration;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionError, TransactionTrait, sea_query::OnConflict,
};

use super::database_now;
use crate::nodes::{
    domain::{CheckIn, CheckInRejection, validate},
    entities::{node, node_sample},
};

/// Agent sample times further than this from the database clock are replaced by it.
const MAX_CLOCK_SKEW: Duration = Duration::minutes(5);

#[derive(Debug, thiserror::Error)]
pub enum RecordCheckInError {
    #[error(transparent)]
    Rejected(#[from] CheckInRejection),
    #[error("check-in persistence failed")]
    Persistence(#[source] DbErr),
}

/// Upsert the node and insert its sample in one transaction. A sample without disk usage carries
/// the node's last known value forward (the agent reads disk every twelfth sample).
pub async fn execute(
    database: &DatabaseConnection,
    cluster_id: &str,
    check_in: CheckIn,
) -> Result<(), RecordCheckInError> {
    validate(&check_in)?;
    let cluster_id = cluster_id.to_owned();
    database
        .transaction::<_, (), DbErr>(|transaction| {
            Box::pin(async move {
                let now = database_now(transaction).await?;
                let row = node::ActiveModel {
                    id: Set(check_in.node_id.clone()),
                    cluster_id: Set(cluster_id),
                    hostname: Set(check_in.hostname),
                    swarm_node_id: Set(check_in.swarm_node_id.filter(|id| !id.is_empty())),
                    advertise_address: Set(check_in.advertise_address),
                    agent_version: Set(check_in.agent_version),
                    docker_version: Set(check_in.docker_version),
                    os: Set(check_in.os),
                    arch: Set(check_in.architecture),
                    cpu_cores: Set(i32::try_from(check_in.cpu_cores).unwrap_or(i32::MAX)),
                    memory_total_bytes: Set(
                        i64::try_from(check_in.memory_total_bytes).unwrap_or(i64::MAX)
                    ),
                    agent_started_at: Set(check_in.agent_started_at),
                    first_seen_at: Set(now),
                    last_check_in_at: Set(now),
                    created_at: Set(now),
                    updated_at: Set(now),
                };
                node::Entity::insert(row)
                    .on_conflict(
                        OnConflict::column(node::Column::Id)
                            .update_columns([
                                node::Column::ClusterId,
                                node::Column::Hostname,
                                node::Column::SwarmNodeId,
                                node::Column::AdvertiseAddress,
                                node::Column::AgentVersion,
                                node::Column::DockerVersion,
                                node::Column::Os,
                                node::Column::Arch,
                                node::Column::CpuCores,
                                node::Column::MemoryTotalBytes,
                                node::Column::AgentStartedAt,
                                node::Column::LastCheckInAt,
                                node::Column::UpdatedAt,
                            ])
                            .to_owned(),
                    )
                    .exec_without_returning(transaction)
                    .await?;

                let Some(sample) = check_in.sample else {
                    return Ok(());
                };
                let disk_used_bytes = match sample.disk_used_bytes {
                    Some(disk) => Some(i64::try_from(disk).unwrap_or(i64::MAX)),
                    None => node_sample::Entity::find()
                        .select_only()
                        .column(node_sample::Column::DiskUsedBytes)
                        .filter(node_sample::Column::NodeId.eq(&check_in.node_id))
                        .filter(node_sample::Column::DiskUsedBytes.is_not_null())
                        .order_by_desc(node_sample::Column::SampledAt)
                        .into_tuple::<Option<i64>>()
                        .one(transaction)
                        .await?
                        .flatten(),
                };
                let sampled_at = if (sample.sampled_at - now).abs() > MAX_CLOCK_SKEW {
                    now
                } else {
                    sample.sampled_at
                };
                let count = |value: u32| i32::try_from(value).unwrap_or(i32::MAX);
                node_sample::Entity::insert(node_sample::ActiveModel {
                    node_id: Set(check_in.node_id),
                    sampled_at: Set(sampled_at),
                    cpu_percent: Set(sample.cpu_percent),
                    memory_used_bytes: Set(
                        i64::try_from(sample.memory_used_bytes).unwrap_or(i64::MAX)
                    ),
                    disk_used_bytes: Set(disk_used_bytes),
                    containers_running: Set(count(sample.containers_running)),
                    containers_paused: Set(count(sample.containers_paused)),
                    containers_exited: Set(count(sample.containers_exited)),
                    net_rx_bytes_per_sec: Set(sample.net_rx_bytes_per_sec),
                    net_tx_bytes_per_sec: Set(sample.net_tx_bytes_per_sec),
                })
                // A retried check-in repeats its sample time; keep the first.
                .on_conflict(
                    OnConflict::columns([
                        node_sample::Column::NodeId,
                        node_sample::Column::SampledAt,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec_without_returning(transaction)
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) | TransactionError::Transaction(error) => {
                RecordCheckInError::Persistence(error)
            }
        })
}
