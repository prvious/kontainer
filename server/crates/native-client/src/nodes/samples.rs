//! `GET /api/v1/cluster/samples`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use super::{SampleSeries, SampleSeriesBody, validate_range};
use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Cluster-wide buckets: mean CPU over contributing nodes, summed figures, and the count of
    /// nodes that contributed.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::InvalidRequest`] for a malformed window or bucket (checked locally
    /// first, then by the controller).
    pub async fn cluster_samples(
        &self,
        access: &AccessCredential,
        window: &str,
        bucket: &str,
        cancellation: &CancellationToken,
    ) -> Result<SampleSeries, NativeClientError> {
        validate_range(window, bucket)?;
        let mut url = self.endpoint("/api/v1/cluster/samples")?;
        url.query_pairs_mut()
            .append_pair("window", window)
            .append_pair("bucket", bucket);
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<SampleSeriesBody>(&body)?.into_series()
    }
}
