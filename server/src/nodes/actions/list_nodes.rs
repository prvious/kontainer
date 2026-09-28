use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    sea_query::{Expr, ExprTrait, Func, Query},
};

use super::database_now;
use crate::nodes::{
    domain::{LatestSample, NodeStatus, NodeView},
    entities::{node, node_sample},
};

/// Every node with its latest sample and a status derived from the database clock. Returns the
/// clock reading as the snapshot time.
pub async fn execute(
    database: &DatabaseConnection,
) -> Result<(Vec<NodeView>, DateTime<Utc>), DbErr> {
    let now = database_now(database).await?;
    let nodes = node::Entity::find()
        .order_by_asc(node::Column::Id)
        .all(database)
        .await?;
    let latest = node_sample::Entity::find()
        .filter(
            Expr::tuple([
                Expr::col(node_sample::Column::NodeId),
                Expr::col(node_sample::Column::SampledAt),
            ])
            .in_subquery(
                Query::select()
                    .column(node_sample::Column::NodeId)
                    .expr(Func::max(Expr::col(node_sample::Column::SampledAt)))
                    .from(node_sample::Entity)
                    .group_by_col(node_sample::Column::NodeId)
                    .to_owned(),
            ),
        )
        .all(database)
        .await?
        .into_iter()
        .map(|sample| (sample.node_id.clone(), sample))
        .collect::<HashMap<_, _>>();

    let views = nodes
        .into_iter()
        .map(|row| {
            let latest_sample = latest.get(&row.id).map(|sample| LatestSample {
                sampled_at: sample.sampled_at,
                cpu_percent: sample.cpu_percent,
                memory_used_bytes: sample.memory_used_bytes,
                disk_used_bytes: sample.disk_used_bytes,
                containers_running: sample.containers_running,
                containers_paused: sample.containers_paused,
                containers_exited: sample.containers_exited,
                net_rx_bytes_per_sec: sample.net_rx_bytes_per_sec,
                net_tx_bytes_per_sec: sample.net_tx_bytes_per_sec,
            });
            NodeView {
                status: NodeStatus::at(row.last_check_in_at, now),
                id: row.id,
                hostname: row.hostname,
                swarm_node_id: row.swarm_node_id,
                advertise_address: row.advertise_address,
                agent_version: row.agent_version,
                docker_version: row.docker_version,
                os: row.os,
                arch: row.arch,
                cpu_cores: row.cpu_cores,
                memory_total_bytes: row.memory_total_bytes,
                agent_started_at: row.agent_started_at,
                first_seen_at: row.first_seen_at,
                last_check_in_at: row.last_check_in_at,
                latest_sample,
            }
        })
        .collect();
    Ok((views, now))
}
