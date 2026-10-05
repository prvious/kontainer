//! Images across the cluster, pull references, and the documents image operations carry.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::rpc::AgentImage;

const MAX_NAME_LEN: usize = 255;
const MAX_TAG_LEN: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReferenceError {
    #[error("the reference is empty or too long")]
    Length,
    #[error("the registry host is not valid")]
    Domain,
    #[error("the repository name is not valid")]
    Name,
    #[error("the tag is not valid")]
    Tag,
    #[error("the digest is not valid")]
    Digest,
}

/// Validate `[host[:port]/]name[:tag|@digest]` against the Docker reference grammar and return
/// it with `:latest` added when it names neither a tag nor a digest (a bare name would pull every
/// tag).
pub fn normalize_reference(value: &str) -> Result<String, ReferenceError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 512 {
        return Err(ReferenceError::Length);
    }
    let (rest, digest) = match value.split_once('@') {
        Some((rest, digest)) => (rest, Some(digest)),
        None => (value, None),
    };
    if let Some(digest) = digest {
        let valid = digest.strip_prefix("sha256:").is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        });
        if !valid {
            return Err(ReferenceError::Digest);
        }
    }
    // A tag follows the last colon only when that colon comes after the last slash.
    let last_slash = rest.rfind('/');
    let (name, tag) = match rest.rfind(':') {
        Some(colon) if last_slash.is_none_or(|slash| colon > slash) => {
            (&rest[..colon], Some(&rest[colon + 1..]))
        }
        _ => (rest, None),
    };
    if let Some(tag) = tag {
        let mut characters = tag.chars();
        let first_ok = characters
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric() || first == '_');
        if !first_ok
            || tag.len() > MAX_TAG_LEN
            || !tag
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "_.-".contains(character))
        {
            return Err(ReferenceError::Tag);
        }
    }
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(ReferenceError::Name);
    }
    let mut components: Vec<&str> = name.split('/').collect();
    let first = components[0];
    let has_domain = components.len() > 1
        && (first.contains('.')
            || first.contains(':')
            || first == "localhost"
            || first.chars().any(|c| c.is_ascii_uppercase()));
    if has_domain {
        if !valid_domain(first) {
            return Err(ReferenceError::Domain);
        }
        components.remove(0);
    }
    if components.is_empty()
        || !components
            .iter()
            .all(|component| valid_path_component(component))
    {
        return Err(ReferenceError::Name);
    }
    let mut normalized = rest.to_owned();
    if tag.is_none() && digest.is_none() {
        normalized.push_str(":latest");
    }
    if let Some(digest) = digest {
        normalized.push('@');
        normalized.push_str(digest);
    }
    Ok(normalized)
}

fn valid_domain(value: &str) -> bool {
    let (host, port) = match value.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (value, None),
    };
    let port_ok = port.is_none_or(|port| {
        !port.is_empty() && port.len() <= 5 && port.bytes().all(|byte| byte.is_ascii_digit())
    });
    port_ok
        && !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
}

/// `[a-z0-9]+` joined by `.`, `_`, `__`, or runs of `-`.
fn valid_path_component(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit()
        || !bytes[bytes.len() - 1].is_ascii_lowercase() && !bytes[bytes.len() - 1].is_ascii_digit()
    {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len()
            && !(bytes[index].is_ascii_lowercase() || bytes[index].is_ascii_digit())
        {
            index += 1;
        }
        let separator = &value[start..index];
        let dashes = separator.bytes().all(|byte| byte == b'-');
        if !(separator == "." || separator == "_" || separator == "__" || dashes) {
            return false;
        }
    }
    true
}

/// One image id across the nodes that have it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRow {
    pub image_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub size_bytes: u64,
    pub created_at_seconds: i64,
    pub used_by: u32,
    pub node_ids: Vec<String>,
}

/// Merge per-node images by id: tags and digests unioned, `usedBy` summed, size counted once.
#[must_use]
pub fn merge_images(per_node: Vec<(String, Vec<AgentImage>)>) -> Vec<ImageRow> {
    let mut rows: BTreeMap<String, (ImageRow, BTreeSet<String>, BTreeSet<String>)> =
        BTreeMap::new();
    for (node_id, images) in per_node {
        for image in images {
            let entry = rows.entry(image.image_id.clone()).or_insert_with(|| {
                (
                    ImageRow {
                        image_id: image.image_id.clone(),
                        repo_tags: Vec::new(),
                        repo_digests: Vec::new(),
                        size_bytes: image.size_bytes,
                        created_at_seconds: image.created_at_seconds,
                        used_by: 0,
                        node_ids: Vec::new(),
                    },
                    BTreeSet::new(),
                    BTreeSet::new(),
                )
            });
            entry.0.used_by = entry.0.used_by.saturating_add(image.used_by);
            entry.0.node_ids.push(node_id.clone());
            entry.1.extend(image.repo_tags);
            entry.2.extend(image.repo_digests);
        }
    }
    let mut merged: Vec<ImageRow> = rows
        .into_values()
        .map(|(mut row, tags, digests)| {
            row.repo_tags = tags.into_iter().collect();
            row.repo_digests = digests.into_iter().collect();
            row.node_ids.sort();
            row
        })
        .collect();
    merged.sort_by(|left, right| {
        right
            .created_at_seconds
            .cmp(&left.created_at_seconds)
            .then_with(|| left.image_id.cmp(&right.image_id))
    });
    merged
}

/// `nodeIds: [...]` or the string `"all"` (every member that has it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NodeSelection {
    All(AllNodes),
    Nodes(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AllNodes {
    All,
}

/// The stored request of an `image.remove` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveImagePayload {
    pub image_id: String,
    pub node_ids: NodeSelection,
    #[serde(default)]
    pub force: bool,
}

/// The stored request of an `image.pull` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullImagePayload {
    /// Normalized: always a tag or a digest.
    pub reference: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodePullStatus {
    Pulling,
    Done,
    Failed,
}

/// One node's pull as the operation detail reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePull {
    pub node_id: String,
    pub status: NodePullStatus,
    pub layers_done: u32,
    pub layers_total: u32,
    /// Bytes downloaded so far across layers.
    pub bytes: u64,
    /// Bytes to download across the layers whose size is known.
    pub total_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Per-layer bookkeeping for one node's pull.
#[derive(Debug, Clone, Default)]
pub struct PullTracker {
    layers: BTreeMap<String, (bool, u64, u64)>,
}

const LAYER_STATUSES: [&str; 8] = [
    "Pulling fs layer",
    "Waiting",
    "Downloading",
    "Verifying Checksum",
    "Download complete",
    "Extracting",
    "Pull complete",
    "Already exists",
];

impl PullTracker {
    /// Fold one daemon progress message into the layer table.
    pub fn record(
        &mut self,
        layer_id: Option<&str>,
        status: &str,
        current: Option<u64>,
        total: Option<u64>,
    ) {
        let Some(layer) = layer_id else { return };
        if !LAYER_STATUSES.contains(&status) {
            return;
        }
        let entry = self.layers.entry(layer.to_owned()).or_insert((false, 0, 0));
        match status {
            "Downloading" => {
                entry.1 = current.unwrap_or(entry.1);
                entry.2 = total.unwrap_or(entry.2);
            }
            "Download complete" | "Extracting" | "Verifying Checksum" => {
                entry.1 = entry.1.max(entry.2)
            }
            "Pull complete" | "Already exists" => {
                entry.0 = true;
                entry.1 = entry.1.max(entry.2);
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn snapshot(
        &self,
        node_id: &str,
        status: NodePullStatus,
        error: Option<String>,
    ) -> NodePull {
        let count = |done: bool| {
            u32::try_from(
                self.layers
                    .values()
                    .filter(|layer| !done || layer.0)
                    .count(),
            )
            .unwrap_or(u32::MAX)
        };
        // A finished pull has every layer on disk, even entries the daemon never marked
        // complete (Docker's containerd store reports manifests and configs as layers).
        let finished = status == NodePullStatus::Done;
        NodePull {
            node_id: node_id.to_owned(),
            status,
            layers_done: count(!finished),
            layers_total: count(false),
            bytes: self.layers.values().map(|layer| layer.1).sum(),
            total_bytes: self.layers.values().map(|layer| layer.2).sum(),
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NodePullStatus, PullTracker, ReferenceError, merge_images, normalize_reference};
    use crate::rpc::AgentImage;

    #[test]
    fn references_follow_the_docker_grammar() {
        for (input, normalized) in [
            ("alpine", "alpine:latest"),
            ("alpine:3.20", "alpine:3.20"),
            ("library/alpine:3.20", "library/alpine:3.20"),
            ("ghcr.io/acme/billing:2.4.0", "ghcr.io/acme/billing:2.4.0"),
            ("localhost:5000/team/app", "localhost:5000/team/app:latest"),
            (
                "registry.example.com:443/a/b/c:v1_2.3-rc",
                "registry.example.com:443/a/b/c:v1_2.3-rc",
            ),
            ("my-org/my__app.name:tag", "my-org/my__app.name:tag"),
            (
                &format!("alpine@sha256:{}", "a".repeat(64)),
                &format!("alpine@sha256:{}", "a".repeat(64)),
            ),
        ] {
            assert_eq!(
                normalize_reference(input).as_deref(),
                Ok(normalized),
                "{input}"
            );
        }
        for (input, error) in [
            ("", ReferenceError::Length),
            ("Alpine", ReferenceError::Name),
            ("alpine:", ReferenceError::Tag),
            ("alpine:-bad", ReferenceError::Tag),
            ("alpine:has space", ReferenceError::Tag),
            ("alpine@sha256:abc", ReferenceError::Digest),
            ("-alpine", ReferenceError::Name),
            ("alpine-", ReferenceError::Name),
            ("a..b/c", ReferenceError::Domain),
            ("bad_host.io:99999999/x", ReferenceError::Domain),
            ("alp!ne", ReferenceError::Name),
            ("ghcr.io/", ReferenceError::Name),
        ] {
            assert_eq!(normalize_reference(input), Err(error), "{input}");
        }
        assert_eq!(
            normalize_reference(&"a".repeat(513)),
            Err(ReferenceError::Length)
        );
    }

    fn image(id: char, tags: &[&str], used_by: u32, created: i64) -> AgentImage {
        AgentImage {
            image_id: format!("sha256:{}", id.to_string().repeat(64)),
            repo_tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            repo_digests: Vec::new(),
            size_bytes: 1000,
            created_at_seconds: created,
            used_by,
        }
    }

    #[test]
    fn images_merge_by_id_across_nodes() {
        let merged = merge_images(vec![
            (
                "manager".to_owned(),
                vec![image('a', &["nginx:1.27"], 2, 10), image('b', &[], 0, 5)],
            ),
            (
                "worker-1".to_owned(),
                vec![image('a', &["nginx:1.27", "nginx:latest"], 1, 10)],
            ),
        ]);
        assert_eq!(merged.len(), 2);
        let nginx = &merged[0];
        assert_eq!(nginx.node_ids, vec!["manager", "worker-1"]);
        assert_eq!(nginx.used_by, 3, "summed");
        assert_eq!(nginx.size_bytes, 1000, "counted once");
        assert_eq!(nginx.repo_tags, vec!["nginx:1.27", "nginx:latest"]);
        assert!(
            merged[1].repo_tags.is_empty(),
            "dangling: no tag on any node"
        );
    }

    #[test]
    fn pull_progress_counts_layers_and_bytes() {
        let mut tracker = PullTracker::default();
        tracker.record(Some("3.20"), "Pulling from library/alpine", None, None);
        tracker.record(Some("l1"), "Pulling fs layer", None, None);
        tracker.record(Some("l2"), "Already exists", None, None);
        tracker.record(Some("l1"), "Downloading", Some(500), Some(2000));
        let pulling = tracker.snapshot("manager", NodePullStatus::Pulling, None);
        assert_eq!((pulling.layers_done, pulling.layers_total), (1, 2));
        assert_eq!((pulling.bytes, pulling.total_bytes), (500, 2000));
        tracker.record(Some("l1"), "Download complete", None, None);
        tracker.record(Some("l1"), "Pull complete", None, None);
        let done = tracker.snapshot("manager", NodePullStatus::Done, None);
        assert_eq!(
            (done.layers_done, done.layers_total, done.bytes),
            (2, 2, 2000)
        );
        let json = serde_json::to_value(&done).unwrap();
        assert_eq!(json["status"], "done");
        assert!(json.get("error").is_none());

        // The containerd store downloads manifests and configs that never report "Pull complete".
        let mut containerd = PullTracker::default();
        containerd.record(Some("index"), "Waiting", None, None);
        containerd.record(Some("manifest"), "Downloading", Some(10), Some(10));
        containerd.record(Some("layer"), "Pull complete", None, None);
        let pulling = containerd.snapshot("manager", NodePullStatus::Pulling, None);
        assert_eq!((pulling.layers_done, pulling.layers_total), (1, 3));
        let done = containerd.snapshot("manager", NodePullStatus::Done, None);
        assert_eq!((done.layers_done, done.layers_total), (3, 3));
    }
}
