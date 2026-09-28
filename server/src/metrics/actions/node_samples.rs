use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    sea_query::{Alias, Expr, Func, Query, SelectStatement},
};

use crate::{
    metrics::domain::{SamplePoint, SampleRange, SampleScope},
    nodes::{actions::database_now, entities::node_sample},
};

/// Averages per `date_bin` bucket for one node, or the cluster: per-node bucket averages first,
/// then the cluster mean CPU, summed figures, and the count of contributing nodes. Every bucket in
/// the range is returned; a bucket without samples has `None` figures.
pub async fn execute(
    database: &DatabaseConnection,
    scope: &SampleScope,
    range: SampleRange,
) -> Result<(Vec<SamplePoint>, DateTime<Utc>), DbErr> {
    let now = database_now(database).await?;
    let (start, end) = range.bounds(now);
    let query = bucket_query(scope, range, start, end);
    let mut found = HashMap::new();
    for row in database.query_all(&query).await? {
        let at: DateTime<Utc> = row.try_get("", "bucket")?;
        let nodes: i64 = row.try_get("", "nodes")?;
        found.insert(
            at,
            SamplePoint {
                at,
                cpu_percent: row.try_get("", "cpu_percent")?,
                memory_used_bytes: row.try_get("", "memory_used_bytes")?,
                disk_used_bytes: row.try_get("", "disk_used_bytes")?,
                containers_running: row.try_get("", "containers_running")?,
                net_rx_bytes_per_sec: row.try_get("", "net_rx_bytes_per_sec")?,
                net_tx_bytes_per_sec: row.try_get("", "net_tx_bytes_per_sec")?,
                nodes: u32::try_from(nodes).unwrap_or(u32::MAX),
            },
        );
    }
    let points = range
        .bucket_starts(now)
        .into_iter()
        .map(|at| found.remove(&at).unwrap_or(SamplePoint::empty(at)))
        .collect();
    Ok((points, now))
}

fn bucket_query(
    scope: &SampleScope,
    range: SampleRange,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> SelectStatement {
    let float = |expr: Expr| Expr::from(Func::cast_as(expr, "float8"));
    // date_bin(interval, ts, origin): SeaQuery has no interval literal, so the interval is the
    // one bound custom fragment; the bucket width is a validated integer.
    let bucket = Func::cust("date_bin")
        .arg(Expr::cust_with_values(
            "make_interval(secs => $1)",
            [range.bucket_seconds() as f64],
        ))
        .arg(Expr::col(node_sample::Column::SampledAt))
        .arg(Expr::val(DateTime::<Utc>::UNIX_EPOCH));

    let mut per_node = Query::select();
    per_node
        .column(node_sample::Column::NodeId)
        .expr_as(bucket, "bucket")
        .expr_as(
            float(Func::avg(Expr::col(node_sample::Column::CpuPercent)).into()),
            "cpu_percent",
        )
        .expr_as(
            float(Func::avg(Expr::col(node_sample::Column::MemoryUsedBytes)).into()),
            "memory_used_bytes",
        )
        .expr_as(
            float(Func::avg(Expr::col(node_sample::Column::DiskUsedBytes)).into()),
            "disk_used_bytes",
        )
        .expr_as(
            float(Func::avg(Expr::col(node_sample::Column::ContainersRunning)).into()),
            "containers_running",
        )
        .expr_as(
            Func::avg(Expr::col(node_sample::Column::NetRxBytesPerSec)),
            "net_rx_bytes_per_sec",
        )
        .expr_as(
            Func::avg(Expr::col(node_sample::Column::NetTxBytesPerSec)),
            "net_tx_bytes_per_sec",
        )
        .from(node_sample::Entity)
        .and_where(node_sample::Column::SampledAt.gte(start))
        .and_where(node_sample::Column::SampledAt.lt(end))
        .group_by_col(node_sample::Column::NodeId)
        .add_group_by([Expr::col("bucket")]);
    if let SampleScope::Node(node_id) = scope {
        per_node.and_where(node_sample::Column::NodeId.eq(node_id.as_str()));
    }

    let column = |name: &'static str| Expr::col((Alias::new("per_node"), Alias::new(name)));
    Query::select()
        .expr_as(column("bucket"), "bucket")
        .expr_as(Func::avg(column("cpu_percent")), "cpu_percent")
        .expr_as(Func::sum(column("memory_used_bytes")), "memory_used_bytes")
        .expr_as(Func::sum(column("disk_used_bytes")), "disk_used_bytes")
        .expr_as(
            Func::sum(column("containers_running")),
            "containers_running",
        )
        .expr_as(
            Func::sum(column("net_rx_bytes_per_sec")),
            "net_rx_bytes_per_sec",
        )
        .expr_as(
            Func::sum(column("net_tx_bytes_per_sec")),
            "net_tx_bytes_per_sec",
        )
        .expr_as(Func::count(column("node_id")), "nodes")
        .from_subquery(per_node.to_owned(), "per_node")
        .add_group_by([column("bucket")])
        .to_owned()
}
