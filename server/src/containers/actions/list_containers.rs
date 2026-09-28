use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use tokio_util::sync::CancellationToken;

use crate::{
    containers::domain::{ContainerItem, MAX_CONTAINERS},
    nodes::{
        actions::list_nodes,
        agents::{AgentDirectory, NodeEndpoint},
        domain::NodeStatus,
        fan_out::{NodeFailure, NodeOutcome, PER_NODE_DEADLINE, fan_out},
    },
    rpc::{AgentClient, AgentClientError, AgentContainerSummary},
};

const PAGE_SIZE: u32 = 500;

#[derive(Debug, thiserror::Error)]
pub enum ListContainersError {
    #[error("no such node")]
    UnknownNode,
    #[error("more than {MAX_CONTAINERS} containers")]
    TooMany,
    #[error("membership persistence failed")]
    Persistence(#[from] DbErr),
}

/// Every container on every answering node, one outcome per member node, and the snapshot time.
pub struct ContainerList {
    pub items: Vec<ContainerItem>,
    pub outcomes: Vec<NodeOutcome<()>>,
    pub snapshot_at: DateTime<Utc>,
}

/// Fan out to the online and degraded members (or the one named) and drain their pages.
/// Offline members are reported `unavailable` without a dial.
pub async fn execute(
    database: &DatabaseConnection,
    directory: &dyn AgentDirectory,
    node_filter: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<ContainerList, ListContainersError> {
    let (nodes, snapshot_at) = list_nodes::execute(database).await?;
    let nodes: Vec<_> = match node_filter {
        Some(node_id) => {
            let selected: Vec<_> = nodes
                .into_iter()
                .filter(|node| node.id == node_id)
                .collect();
            if selected.is_empty() {
                return Err(ListContainersError::UnknownNode);
            }
            selected
        }
        None => nodes,
    };
    let names: std::collections::HashMap<_, _> = nodes
        .iter()
        .map(|node| (node.id.clone(), node.hostname.clone()))
        .collect();
    let (reachable, offline): (Vec<_>, Vec<_>) = nodes
        .into_iter()
        .partition(|node| node.status != NodeStatus::Offline);
    let endpoints = reachable
        .into_iter()
        .map(|node| NodeEndpoint {
            node_id: node.id,
            advertise_address: node.advertise_address,
        })
        .collect();
    let outcomes = fan_out(endpoints, directory, PER_NODE_DEADLINE, |client| {
        drain(client, cancellation)
    })
    .await;

    let mut items = Vec::new();
    let mut summary = Vec::new();
    for outcome in outcomes {
        let result = match outcome.result {
            Ok(containers) => {
                let name = names.get(&outcome.node_id).cloned().unwrap_or_default();
                items.extend(
                    containers
                        .into_iter()
                        .map(|container| item(&outcome.node_id, &name, container)),
                );
                if items.len() > MAX_CONTAINERS {
                    return Err(ListContainersError::TooMany);
                }
                Ok(())
            }
            Err(failure) => Err(failure),
        };
        summary.push(NodeOutcome {
            node_id: outcome.node_id,
            result,
        });
    }
    summary.extend(offline.into_iter().map(|node| NodeOutcome {
        node_id: node.id,
        result: Err(NodeFailure::Unavailable),
    }));
    Ok(ContainerList {
        items,
        outcomes: summary,
        snapshot_at,
    })
}

/// Every page of one node, stopping past the cluster cap so a huge node cannot exhaust memory.
async fn drain(
    client: Arc<dyn AgentClient>,
    cancellation: &CancellationToken,
) -> Result<Vec<AgentContainerSummary>, AgentClientError> {
    let mut containers = Vec::new();
    let mut token = String::new();
    loop {
        let page = client
            .list_containers(PAGE_SIZE, token, cancellation)
            .await?;
        containers.extend(page.containers);
        match page.next_page_token {
            Some(next) if containers.len() <= MAX_CONTAINERS => token = next,
            _ => return Ok(containers),
        }
    }
}

fn timestamp(seconds: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(seconds, 0)
}

fn item(node_id: &str, node_name: &str, container: AgentContainerSummary) -> ContainerItem {
    ContainerItem {
        node_id: node_id.to_owned(),
        node_name: node_name.to_owned(),
        created_at: timestamp(container.created_at_seconds).unwrap_or(DateTime::UNIX_EPOCH),
        started_at: container.started_at_seconds.and_then(timestamp),
        finished_at: container.finished_at_seconds.and_then(timestamp),
        container_id: container.container_id,
        name: container.display_name,
        image: container.image_reference,
        state: container.state,
        status: container.status,
        ports: container.published_ports,
        service_name: container.service_name,
        stack_namespace: container.stack_namespace,
        task_id: container.task_id,
        cpu_percent: container.cpu_percent,
        memory_used_bytes: container.memory_used_bytes,
        memory_limit_bytes: container.memory_limit_bytes,
    }
}
