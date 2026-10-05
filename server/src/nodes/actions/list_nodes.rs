use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryOrder,
    sea_query::{Asterisk, Expr, ExprTrait, JoinType, Order, Query},
};

use super::database_now;
use crate::nodes::{
    domain::{LatestSample, NodeStatus, NodeView, ONLINE_WITHIN_SECONDS},
    entities::{node, node_sample},
};

const LATEST_SAMPLE: &str = "latest_sample";

/// Whether a sample still describes the node: it was taken within the online window.
///
/// ponytail: `sampled_at` is the agent's clock (replaced only past 5 minutes of skew) and `now`
/// is the database's, so agent clocks must sit within 15 s of the database clock or a healthy
/// node reads degraded (ADR 0037). Swarm already needs synced clocks; stamp samples on arrival
/// if that ever stops holding.
fn is_fresh(sampled_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    (now - sampled_at).num_milliseconds() <= ONLINE_WITHIN_SECONDS * 1000
}

/// An agent that keeps checking in without a fresh sample can't read its Docker (it sends none
/// then), so it is `degraded` rather than `online`.
fn status(
    last_check_in_at: DateTime<Utc>,
    has_fresh_sample: bool,
    now: DateTime<Utc>,
) -> NodeStatus {
    match NodeStatus::at(last_check_in_at, now) {
        NodeStatus::Online if !has_fresh_sample => NodeStatus::Degraded,
        status => status,
    }
}

/// Every node with its latest sample, when fresh, and a status derived from the database clock.
/// Returns the clock reading as the snapshot time.
pub async fn execute(
    database: &DatabaseConnection,
) -> Result<(Vec<NodeView>, DateTime<Utc>), DbErr> {
    let now = database_now(database).await?;
    let nodes = node::Entity::find()
        .order_by_asc(node::Column::Id)
        .all(database)
        .await?;
    // Each node's newest sample through the `(node_id, sampled_at)` primary key: one backward
    // index probe per node, however much history retention keeps.
    let newest_per_node = Query::select()
        .column((LATEST_SAMPLE, Asterisk))
        .from(node::Entity)
        .join_lateral(
            JoinType::InnerJoin,
            Query::select()
                .column(Asterisk)
                .from(node_sample::Entity)
                .and_where(
                    Expr::col((node_sample::Entity, node_sample::Column::NodeId))
                        .equals((node::Entity, node::Column::Id)),
                )
                .order_by(node_sample::Column::SampledAt, Order::Desc)
                .limit(1)
                .to_owned(),
            LATEST_SAMPLE,
            Expr::val(true),
        )
        .to_owned();
    let latest = node_sample::Model::find_by_statement(
        database.get_database_backend().build(&newest_per_node),
    )
    .all(database)
    .await?
    .into_iter()
    .map(|sample| (sample.node_id.clone(), sample))
    .collect::<HashMap<_, _>>();

    let views = nodes
        .into_iter()
        .map(|row| {
            // A stale sample is left out: old figures must not read as live.
            let latest_sample = latest
                .get(&row.id)
                .filter(|sample| is_fresh(sample.sampled_at, now))
                .map(|sample| LatestSample {
                    cpu_percent: sample.cpu_percent,
                    memory_used_bytes: sample.memory_used_bytes,
                    containers_running: sample.containers_running,
                    containers_paused: sample.containers_paused,
                    containers_exited: sample.containers_exited,
                    net_rx_bytes_per_sec: sample.net_rx_bytes_per_sec,
                    net_tx_bytes_per_sec: sample.net_tx_bytes_per_sec,
                });
            NodeView {
                status: status(row.last_check_in_at, latest_sample.is_some(), now),
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
                last_check_in_at: row.last_check_in_at,
                latest_sample,
            }
        })
        .collect();
    Ok((views, now))
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{is_fresh, status};
    use crate::nodes::domain::NodeStatus;

    #[test]
    fn a_sample_older_than_the_online_window_is_not_live() {
        let now = Utc::now();
        assert!(is_fresh(now - Duration::seconds(15), now));
        assert!(!is_fresh(now - Duration::seconds(16), now));
        assert!(!is_fresh(now - Duration::hours(3), now));
    }

    #[test]
    fn an_agent_checking_in_without_a_fresh_sample_is_degraded() {
        let now = Utc::now();
        let checked_in = now - Duration::seconds(2);
        assert_eq!(status(checked_in, true, now), NodeStatus::Online);
        assert_eq!(status(checked_in, false, now), NodeStatus::Degraded);
        let silent = now - Duration::seconds(61);
        assert_eq!(status(silent, false, now), NodeStatus::Offline);
        assert_eq!(status(silent, true, now), NodeStatus::Offline);
    }
}
