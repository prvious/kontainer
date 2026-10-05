//! Networks across the cluster: swarm-scope ones from the manager with per-node endpoint counts,
//! local-scope ones per node.

use std::collections::{BTreeMap, BTreeSet};

use kontainer_engine::NetworkSummary;
use serde::{Deserialize, Serialize};

use crate::rpc::AgentNetwork;

/// One row: a swarm-scope network once for the cluster, a local one once per node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRow {
    pub id: String,
    pub name: String,
    pub driver: String,
    /// `swarm` or `local`.
    pub scope: String,
    pub subnets: Vec<String>,
    /// Running containers attached, summed across nodes.
    pub containers: u32,
    pub created_at_seconds: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub attachable: bool,
    pub builtin: bool,
    /// Local: its node. Swarm: the nodes with endpoints on it.
    pub node_ids: Vec<String>,
}

impl NetworkRow {
    #[must_use]
    pub fn is_swarm(&self) -> bool {
        self.scope == "swarm"
    }

    fn from_agent(network: AgentNetwork, node_ids: Vec<String>) -> Self {
        Self {
            id: network.id,
            name: network.name,
            driver: network.driver,
            scope: if network.scope == "swarm" {
                "swarm".to_owned()
            } else {
                "local".to_owned()
            },
            subnets: network.subnets,
            containers: network.containers,
            created_at_seconds: network.created_at_seconds,
            labels: network.labels,
            attachable: network.attachable,
            builtin: network.builtin,
            node_ids,
        }
    }

    fn from_manager(network: NetworkSummary) -> Self {
        Self {
            builtin: network.builtin(),
            id: network.id,
            name: network.name,
            driver: network.driver,
            scope: "swarm".to_owned(),
            subnets: network.subnets,
            containers: 0,
            created_at_seconds: network.created_at_seconds,
            labels: network.labels,
            attachable: network.attachable,
            node_ids: Vec::new(),
        }
    }
}

/// Merge the manager's cluster-wide list with each answering node's list.
///
/// Swarm-scope networks come from the manager when it answered (a worker only lists one while a
/// task uses it), with every node's local endpoint count summed and the nodes that have
/// endpoints named. Without the manager they come from the nodes that list them. Local networks
/// are one row per node; the manager's own local networks come from its agent, not twice.
#[must_use]
pub fn merge_networks(
    manager: Option<Vec<NetworkSummary>>,
    per_node: Vec<(String, Vec<AgentNetwork>)>,
) -> Vec<NetworkRow> {
    let mut swarm: BTreeMap<String, NetworkRow> = BTreeMap::new();
    let manager_answered = manager.is_some();
    if let Some(networks) = manager {
        for network in networks
            .into_iter()
            .filter(|network| network.scope == "swarm")
        {
            swarm.insert(network.id.clone(), NetworkRow::from_manager(network));
        }
    }
    let mut endpoints: BTreeMap<String, (u32, BTreeSet<String>)> = BTreeMap::new();
    let mut rows = Vec::new();
    for (node_id, networks) in per_node {
        for network in networks {
            if network.scope == "swarm" {
                let entry = endpoints.entry(network.id.clone()).or_default();
                entry.0 = entry.0.saturating_add(network.containers);
                if network.containers > 0 {
                    entry.1.insert(node_id.clone());
                }
                if !manager_answered {
                    swarm
                        .entry(network.id.clone())
                        .or_insert_with(|| NetworkRow::from_agent(network, Vec::new()));
                }
            } else {
                rows.push(NetworkRow::from_agent(network, vec![node_id.clone()]));
            }
        }
    }
    for (id, mut row) in swarm {
        if let Some((containers, nodes)) = endpoints.remove(&id) {
            row.containers = containers;
            row.node_ids = nodes.into_iter().collect();
        }
        rows.push(row);
    }
    rows.sort_by(|left, right| (&left.name, &left.node_ids).cmp(&(&right.name, &right.node_ids)));
    rows
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNetworkPayload {
    pub name: String,
    /// `overlay` (on the manager) or `bridge` (on `node_id`).
    pub driver: String,
    pub attachable: bool,
    pub node_id: Option<String>,
    pub subnet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveNetworkPayload {
    pub network_id: String,
    pub name: String,
    /// Present for a local network; absent for a swarm-scope one (removed on the manager).
    pub node_id: Option<String>,
}

/// The message a refused removal carries: who is attached.
#[must_use]
pub fn in_use_message(name: &str, containers: &[String]) -> String {
    format!(
        "Can't remove {name}: {} {} attached ({}). Remove or move {} first.",
        containers.len(),
        if containers.len() == 1 {
            "container is"
        } else {
            "containers are"
        },
        containers.join(", "),
        if containers.len() == 1 { "it" } else { "them" }
    )
}

#[cfg(test)]
mod tests {
    use kontainer_engine::NetworkSummary;

    use super::{in_use_message, merge_networks};
    use crate::rpc::AgentNetwork;

    fn agent(id: char, name: &str, scope: &str, containers: u32) -> AgentNetwork {
        AgentNetwork {
            id: id.to_string().repeat(64),
            name: name.to_owned(),
            driver: if scope == "swarm" {
                "overlay"
            } else {
                "bridge"
            }
            .to_owned(),
            scope: scope.to_owned(),
            containers,
            builtin: name == "bridge" || name == "ingress",
            ..AgentNetwork::default()
        }
    }

    fn manager(id: char, name: &str) -> NetworkSummary {
        NetworkSummary {
            id: id.to_string().repeat(64),
            name: name.to_owned(),
            driver: "overlay".to_owned(),
            scope: "swarm".to_owned(),
            attachable: true,
            ..NetworkSummary::default()
        }
    }

    #[test]
    fn swarm_networks_merge_once_with_counts_and_local_ones_stay_per_node() {
        let rows = merge_networks(
            Some(vec![
                manager('s', "toolbox_kontainer"),
                manager('i', "ingress"),
                manager('e', "unused"),
            ]),
            vec![
                (
                    "manager".to_owned(),
                    vec![
                        agent('b', "bridge", "local", 0),
                        agent('s', "toolbox_kontainer", "swarm", 2),
                    ],
                ),
                (
                    "worker-1".to_owned(),
                    vec![
                        agent('b', "bridge", "local", 0),
                        agent('s', "toolbox_kontainer", "swarm", 3),
                    ],
                ),
                (
                    "worker-2".to_owned(),
                    vec![
                        agent('l', "local-net", "local", 1),
                        agent('i', "ingress", "swarm", 0),
                    ],
                ),
            ],
        );
        let summary: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    row.name.as_str(),
                    row.scope.as_str(),
                    row.containers,
                    row.node_ids.join(","),
                    row.builtin,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("bridge", "local", 0, "manager".to_owned(), true),
                ("bridge", "local", 0, "worker-1".to_owned(), true),
                ("ingress", "swarm", 0, String::new(), true),
                ("local-net", "local", 1, "worker-2".to_owned(), false),
                (
                    "toolbox_kontainer",
                    "swarm",
                    5,
                    "manager,worker-1".to_owned(),
                    false
                ),
                ("unused", "swarm", 0, String::new(), false),
            ]
        );
    }

    #[test]
    fn without_the_manager_swarm_networks_come_from_the_nodes_once() {
        let rows = merge_networks(
            None,
            vec![
                (
                    "worker-1".to_owned(),
                    vec![agent('s', "toolbox_kontainer", "swarm", 1)],
                ),
                (
                    "worker-2".to_owned(),
                    vec![agent('s', "toolbox_kontainer", "swarm", 2)],
                ),
            ],
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].containers, rows[0].node_ids.clone()),
            (3, vec!["worker-1".to_owned(), "worker-2".to_owned()])
        );
    }

    #[test]
    fn the_in_use_message_names_the_containers() {
        assert_eq!(
            in_use_message("app-network", &["web.1".to_owned(), "api.1".to_owned()]),
            "Can't remove app-network: 2 containers are attached (web.1, api.1). Remove or move them first."
        );
    }
}
