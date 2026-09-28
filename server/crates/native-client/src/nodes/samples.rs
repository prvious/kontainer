//! `GET /api/v1/nodes/{nodeId}/samples` and `GET /api/v1/cluster/samples`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{SampleSeries, SampleSeriesBody, validate_range};
use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::containers::NodeId;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Bucketed averages for one node.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::InvalidRequest`] for a malformed window or bucket (checked locally
    /// first, then by the controller), [`NativeClientError::NotFound`] for an unknown node.
    pub async fn node_samples(
        &self,
        access: &AccessCredential,
        node_id: &NodeId,
        window: &str,
        bucket: &str,
        cancellation: &CancellationToken,
    ) -> Result<SampleSeries, NativeClientError> {
        validate_range(window, bucket)?;
        let mut url = self.endpoint("/api/v1/nodes")?;
        url.path_segments_mut()
            .map_err(|_| NativeClientError::InvalidConfiguration)?
            .push(node_id.as_str())
            .push("samples");
        self.sample_series(access, url, window, bucket, cancellation)
            .await
    }

    /// Cluster-wide buckets: mean CPU over contributing nodes, summed figures, and the count of
    /// nodes that contributed.
    pub async fn cluster_samples(
        &self,
        access: &AccessCredential,
        window: &str,
        bucket: &str,
        cancellation: &CancellationToken,
    ) -> Result<SampleSeries, NativeClientError> {
        validate_range(window, bucket)?;
        let url = self.endpoint("/api/v1/cluster/samples")?;
        self.sample_series(access, url, window, bucket, cancellation)
            .await
    }

    async fn sample_series(
        &self,
        access: &AccessCredential,
        mut url: Url,
        window: &str,
        bucket: &str,
        cancellation: &CancellationToken,
    ) -> Result<SampleSeries, NativeClientError> {
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
        decode_json::<SampleSeriesBody>(&body)?.into_series(window, bucket)
    }
}
