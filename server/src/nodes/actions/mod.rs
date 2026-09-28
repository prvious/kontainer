pub mod cluster_summary;
pub mod list_cluster_nodes;
pub mod list_nodes;
pub mod record_check_in;

use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DbErr,
    sea_query::{Expr, Query},
};

/// The database clock, so writes and status reads share one notion of "now".
pub(crate) async fn database_now(database: &impl ConnectionTrait) -> Result<DateTime<Utc>, DbErr> {
    let query = Query::select()
        .expr_as(Expr::current_timestamp(), "now")
        .to_owned();
    database
        .query_one(&query)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("now() returned no row".to_owned()))?
        .try_get::<DateTime<Utc>>("", "now")
}
