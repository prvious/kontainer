//! `GET /api/v1/nodes/configured`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use crate::{
    client::{NativeClient, RetryPolicy, decode_json},
    containers::NodeId,
    credentials::AccessCredential,
    error::NativeClientError,
};

impl NativeClient {
    /// Discover the statically configured node without contacting the agent.
    pub async fn discover_node(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<Option<NodeId>, NativeClientError> {
        let url = self.endpoint("/api/v1/nodes/configured")?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        let value: serde_json::Value = decode_json(&body)?;
        let fields = value
            .as_object()
            .ok_or(NativeClientError::InvalidResponse)?;
        if fields.len() != 1 {
            return Err(NativeClientError::InvalidResponse);
        }
        match fields.get("nodeId") {
            Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(node_id)) => NodeId::parse(node_id)
                .map(Some)
                .map_err(|_| NativeClientError::InvalidResponse),
            _ => Err(NativeClientError::InvalidResponse),
        }
    }
}
