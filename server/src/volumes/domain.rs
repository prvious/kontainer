//! Node-scoped volumes and the documents volume operations carry.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::rpc::AgentVolume;

/// The only driver offered in this phase.
pub const LOCAL_DRIVER: &str = "local";

/// One volume on one node: the same name on two nodes is two rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeRow {
    pub node_id: String,
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created_at_seconds: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub used_by: u32,
    pub users: Vec<String>,
    pub size_bytes: Option<u64>,
}

impl VolumeRow {
    #[must_use]
    pub fn from_agent(node_id: &str, volume: AgentVolume) -> Self {
        Self {
            node_id: node_id.to_owned(),
            name: volume.name,
            driver: volume.driver,
            mountpoint: volume.mountpoint,
            created_at_seconds: volume.created_at_seconds,
            labels: volume.labels,
            used_by: volume.used_by,
            users: volume.users,
            size_bytes: volume.size_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVolumePayload {
    pub node_id: String,
    pub name: String,
    pub driver: String,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveVolumePayload {
    pub node_id: String,
    pub name: String,
}

/// The message a refused removal carries: who mounts the volume.
#[must_use]
pub fn in_use_message(users: &[String]) -> String {
    format!(
        "In use by {}. Remove {} first; Docker refuses to remove a mounted volume, even with force.",
        users.join(", "),
        if users.len() == 1 {
            "that container"
        } else {
            "those containers"
        }
    )
}

#[cfg(test)]
mod tests {
    use super::in_use_message;

    #[test]
    fn the_in_use_message_names_every_container() {
        assert_eq!(
            in_use_message(&["db.1".to_owned()]),
            "In use by db.1. Remove that container first; Docker refuses to remove a mounted volume, even with force."
        );
        assert!(
            in_use_message(&["a".to_owned(), "b".to_owned()])
                .starts_with("In use by a, b. Remove those containers")
        );
    }
}
