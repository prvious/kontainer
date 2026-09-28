//! Fan a read out to many nodes with an explicit outcome per node.
//!
//! One slow or broken node never fails the call: each gets its own deadline, at most eight run at
//! once, and the caller receives one [`NodeOutcome`] per node in input order.

use std::{future::Future, sync::Arc, time::Duration};

use chrono::{DateTime, SecondsFormat, Utc};
use futures_util::{StreamExt, stream};
use serde::Serialize;
use utoipa::ToSchema;

use super::agents::{AgentDirectory, NodeEndpoint};
use crate::rpc::{AgentClient, AgentClientError};

pub const FAN_OUT_CONCURRENCY: usize = 8;
pub const PER_NODE_DEADLINE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeFailure {
    Unavailable,
    Timeout,
    /// A stable, redacted code such as `invalid_response`.
    Error(&'static str),
}

impl From<AgentClientError> for NodeFailure {
    fn from(error: AgentClientError) -> Self {
        match error {
            AgentClientError::Unavailable | AgentClientError::Cancelled => Self::Unavailable,
            AgentClientError::TimedOut => Self::Timeout,
            AgentClientError::IdentityMismatch => Self::Error("identity_mismatch"),
            AgentClientError::InvalidResponse => Self::Error("invalid_response"),
            AgentClientError::InvalidPagination => Self::Error("invalid_pagination"),
            AgentClientError::NotFound => Self::Error("not_found"),
            AgentClientError::AlreadyInState => Self::Error("already_in_state"),
            AgentClientError::ContainerRunning => Self::Error("container_running"),
            AgentClientError::EngineError => Self::Error("engine_error"),
            AgentClientError::ImageInUse => Self::Error("image_in_use"),
            AgentClientError::VolumeExists => Self::Error("volume_exists"),
            AgentClientError::BuiltinNetwork => Self::Error("builtin_network"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeOutcome<T> {
    pub node_id: String,
    pub result: Result<T, NodeFailure>,
}

/// Run `read` against every node. The deadline bounds each node, so the whole call is bounded by
/// `deadline × ceil(nodes / 8)`.
pub async fn fan_out<T, F, Fut>(
    nodes: Vec<NodeEndpoint>,
    directory: &dyn AgentDirectory,
    deadline: Duration,
    read: F,
) -> Vec<NodeOutcome<T>>
where
    F: Fn(Arc<dyn AgentClient>) -> Fut,
    Fut: Future<Output = Result<T, AgentClientError>>,
{
    let read = &read;
    let mut outcomes = stream::iter(nodes.into_iter().enumerate())
        .map(|(index, node)| async move {
            let result = match directory.client(&node) {
                Err(error) => Err(error.into()),
                Ok(client) => match tokio::time::timeout(deadline, read(client)).await {
                    Ok(result) => result.map_err(NodeFailure::from),
                    Err(_) => Err(NodeFailure::Timeout),
                },
            };
            (
                index,
                NodeOutcome {
                    node_id: node.node_id,
                    result,
                },
            )
        })
        .buffer_unordered(FAN_OUT_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    outcomes.sort_by_key(|(index, _)| *index);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeOutcomeStatus {
    Ok,
    Timeout,
    Unavailable,
    Error,
}

/// Per-node result shared by every fan-out route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeOutcomeResponse {
    pub node_id: String,
    pub status: NodeOutcomeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// `{ items, nodes, snapshotAt }`: the public shape of every fan-out route.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanOutResponse<T: Serialize> {
    pub items: Vec<T>,
    pub nodes: Vec<NodeOutcomeResponse>,
    pub snapshot_at: String,
}

impl<T: Serialize> FanOutResponse<T> {
    /// Flatten ok outcomes into `items` with `each`; every node reports its status.
    pub fn from_outcomes<V>(
        outcomes: Vec<NodeOutcome<V>>,
        snapshot_at: DateTime<Utc>,
        mut each: impl FnMut(&str, V) -> Vec<T>,
    ) -> Self {
        let mut items = Vec::new();
        let mut nodes = Vec::with_capacity(outcomes.len());
        for outcome in outcomes {
            let (status, message) = match outcome.result {
                Ok(value) => {
                    items.extend(each(&outcome.node_id, value));
                    (NodeOutcomeStatus::Ok, None)
                }
                Err(NodeFailure::Timeout) => (NodeOutcomeStatus::Timeout, None),
                Err(NodeFailure::Unavailable) => (NodeOutcomeStatus::Unavailable, None),
                Err(NodeFailure::Error(code)) => (NodeOutcomeStatus::Error, Some(code.to_owned())),
            };
            nodes.push(NodeOutcomeResponse {
                node_id: outcome.node_id,
                status,
                message,
            });
        }
        Self {
            items,
            nodes,
            snapshot_at: snapshot_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use tokio_util::sync::CancellationToken;

    use super::{FanOutResponse, NodeFailure, fan_out};
    use crate::{
        nodes::agents::{AgentDirectory, NodeEndpoint},
        rpc::{AgentClient, AgentClientError, AgentContainerPage, AgentDescription},
    };

    #[derive(Debug)]
    enum Behaviour {
        Answer,
        Hang,
        Refuse,
    }

    #[derive(Debug)]
    struct FakeAgent {
        node_id: String,
        behaviour: Behaviour,
    }

    #[async_trait::async_trait]
    impl AgentClient for FakeAgent {
        async fn describe(
            &self,
            _: &CancellationToken,
        ) -> Result<AgentDescription, AgentClientError> {
            match self.behaviour {
                Behaviour::Answer => Ok(AgentDescription {
                    node_id: self.node_id.clone(),
                    agent_version: "test".to_owned(),
                    architecture: "test".to_owned(),
                    docker_connected: true,
                }),
                Behaviour::Hang => std::future::pending().await,
                Behaviour::Refuse => Err(AgentClientError::Unavailable),
            }
        }

        async fn list_containers(
            &self,
            _: u32,
            _: String,
            _: &CancellationToken,
        ) -> Result<AgentContainerPage, AgentClientError> {
            Err(AgentClientError::InvalidResponse)
        }

        fn node_id(&self) -> &str {
            &self.node_id
        }
    }

    #[derive(Debug)]
    struct FakeDirectory;

    impl AgentDirectory for FakeDirectory {
        fn client(&self, node: &NodeEndpoint) -> Result<Arc<dyn AgentClient>, AgentClientError> {
            let behaviour = match node.node_id.as_str() {
                "ok" => Behaviour::Answer,
                "slow" => Behaviour::Hang,
                "down" => Behaviour::Refuse,
                _ => return Err(AgentClientError::Unavailable),
            };
            Ok(Arc::new(FakeAgent {
                node_id: node.node_id.clone(),
                behaviour,
            }))
        }
    }

    fn endpoint(node_id: &str) -> NodeEndpoint {
        NodeEndpoint {
            node_id: node_id.to_owned(),
            advertise_address: "127.0.0.1:1".to_owned(),
        }
    }

    #[tokio::test]
    async fn one_ok_one_timeout_one_unavailable_yield_three_outcomes() {
        let started = Instant::now();
        let outcomes = fan_out(
            vec![endpoint("ok"), endpoint("slow"), endpoint("down")],
            &FakeDirectory,
            Duration::from_millis(100),
            |client| async move {
                let cancellation = CancellationToken::new();
                client.describe(&cancellation).await.map(|d| d.node_id)
            },
        )
        .await;
        assert!(
            started.elapsed() < Duration::from_millis(600),
            "bounded by the per-node deadline, not the slow node"
        );
        let results: Vec<_> = outcomes
            .iter()
            .map(|outcome| (outcome.node_id.as_str(), outcome.result.clone()))
            .collect();
        assert_eq!(
            results,
            vec![
                ("ok", Ok("ok".to_owned())),
                ("slow", Err(NodeFailure::Timeout)),
                ("down", Err(NodeFailure::Unavailable)),
            ]
        );

        let response = FanOutResponse::from_outcomes(outcomes, chrono::Utc::now(), |node, id| {
            vec![format!("{node}:{id}")]
        });
        let body = serde_json::to_value(&response).unwrap();
        assert_eq!(body["items"], serde_json::json!(["ok:ok"]));
        assert_eq!(
            body["nodes"],
            serde_json::json!([
                {"nodeId": "ok", "status": "ok"},
                {"nodeId": "slow", "status": "timeout"},
                {"nodeId": "down", "status": "unavailable"},
            ])
        );
        assert!(body["snapshotAt"].as_str().unwrap().ends_with('Z'));
    }

    #[tokio::test]
    async fn concurrency_is_bounded_to_eight() {
        let nodes: Vec<_> = (0..16).map(|_| endpoint("slow")).collect();
        let started = Instant::now();
        let outcomes = fan_out(
            nodes,
            &FakeDirectory,
            Duration::from_millis(100),
            |client| async move { client.describe(&CancellationToken::new()).await },
        )
        .await;
        assert_eq!(outcomes.len(), 16);
        // Two waves of eight hanging nodes.
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert!(
            outcomes
                .iter()
                .all(|outcome| outcome.result == Err(NodeFailure::Timeout))
        );
    }

    #[test]
    fn agent_errors_become_redacted_failures() {
        assert_eq!(
            NodeFailure::from(AgentClientError::IdentityMismatch),
            NodeFailure::Error("identity_mismatch")
        );
        assert_eq!(
            NodeFailure::from(AgentClientError::Cancelled),
            NodeFailure::Unavailable
        );
    }
}
