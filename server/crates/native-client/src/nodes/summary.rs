//! `GET /api/v1/cluster/summary`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use super::{ClusterSummary, ClusterSummaryBody};
use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Node counts by health, manager reachability, quorum, and load. A read-only GET, so a
    /// classified pre-response transient failure is retried once inside the deadline.
    pub async fn cluster_summary(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<ClusterSummary, NativeClientError> {
        let url = self.endpoint("/api/v1/cluster/summary")?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<ClusterSummaryBody>(&body)?.into_summary()
    }
}
