use std::{error::Error, fmt};

use x509_parser::{extensions::GeneralName, parse_x509_certificate};

const TRUST_DOMAIN: &str = "kontainer.internal";
const MAX_ID_LENGTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadKind {
    Controller,
    NodeAgent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkloadIdentity {
    Controller {
        cluster_id: String,
        instance_id: String,
    },
    NodeAgent {
        cluster_id: String,
        node_id: String,
    },
}

impl WorkloadIdentity {
    pub fn from_certificate_der(certificate_der: &[u8]) -> Result<Self, IdentityError> {
        let (remainder, certificate) = parse_x509_certificate(certificate_der)
            .map_err(|_| IdentityError::MalformedCertificate)?;
        if !remainder.is_empty() {
            return Err(IdentityError::MalformedCertificate);
        }

        let subject_alternative_name = certificate
            .subject_alternative_name()
            .map_err(|_| IdentityError::MalformedCertificate)?
            .ok_or(IdentityError::MissingWorkloadIdentity)?;

        let mut identities = subject_alternative_name
            .value
            .general_names
            .iter()
            .filter_map(|name| match name {
                GeneralName::URI(uri) => Some(parse_identity_uri(uri)),
                _ => None,
            });

        let identity = identities
            .next()
            .ok_or(IdentityError::MissingWorkloadIdentity)??;
        if identities.next().is_some() {
            return Err(IdentityError::AmbiguousWorkloadIdentity);
        }

        Ok(identity)
    }

    #[must_use]
    pub fn kind(&self) -> WorkloadKind {
        match self {
            Self::Controller { .. } => WorkloadKind::Controller,
            Self::NodeAgent { .. } => WorkloadKind::NodeAgent,
        }
    }

    #[must_use]
    pub fn cluster_id(&self) -> &str {
        match self {
            Self::Controller { cluster_id, .. } | Self::NodeAgent { cluster_id, .. } => cluster_id,
        }
    }

    #[must_use]
    pub fn node_id(&self) -> Option<&str> {
        match self {
            Self::NodeAgent { node_id, .. } => Some(node_id),
            Self::Controller { .. } => None,
        }
    }

    #[must_use]
    pub fn canonical_uri(&self) -> String {
        match self {
            Self::Controller {
                cluster_id,
                instance_id,
            } => format!("spiffe://{TRUST_DOMAIN}/swarm/{cluster_id}/controller/{instance_id}"),
            Self::NodeAgent {
                cluster_id,
                node_id,
            } => format!("spiffe://{TRUST_DOMAIN}/swarm/{cluster_id}/node/{node_id}/agent"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPeerIdentity(WorkloadIdentity);

impl AuthenticatedPeerIdentity {
    pub(crate) fn new(identity: WorkloadIdentity) -> Self {
        Self(identity)
    }

    #[must_use]
    pub fn workload(&self) -> &WorkloadIdentity {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    MalformedCertificate,
    MissingWorkloadIdentity,
    AmbiguousWorkloadIdentity,
    InvalidWorkloadIdentity,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MalformedCertificate => "peer certificate is malformed",
            Self::MissingWorkloadIdentity => "peer certificate has no workload identity",
            Self::AmbiguousWorkloadIdentity => {
                "peer certificate has more than one workload identity"
            }
            Self::InvalidWorkloadIdentity => "peer certificate has an invalid workload identity",
        })
    }
}

impl Error for IdentityError {}

fn parse_identity_uri(uri: &str) -> Result<WorkloadIdentity, IdentityError> {
    let prefix = format!("spiffe://{TRUST_DOMAIN}/swarm/");
    let path = uri
        .strip_prefix(&prefix)
        .ok_or(IdentityError::InvalidWorkloadIdentity)?;
    if path.contains(['%', '?', '#', '\\']) {
        return Err(IdentityError::InvalidWorkloadIdentity);
    }

    let segments = path.split('/').collect::<Vec<_>>();
    match segments.as_slice() {
        [cluster_id, "controller", instance_id]
            if valid_identity_segment(cluster_id) && valid_identity_segment(instance_id) =>
        {
            Ok(WorkloadIdentity::Controller {
                cluster_id: (*cluster_id).to_owned(),
                instance_id: (*instance_id).to_owned(),
            })
        }
        [cluster_id, "node", node_id, "agent"]
            if valid_identity_segment(cluster_id) && valid_identity_segment(node_id) =>
        {
            Ok(WorkloadIdentity::NodeAgent {
                cluster_id: (*cluster_id).to_owned(),
                node_id: (*node_id).to_owned(),
            })
        }
        _ => Err(IdentityError::InvalidWorkloadIdentity),
    }
}

fn valid_identity_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::{IdentityError, WorkloadIdentity, WorkloadKind, parse_identity_uri};

    #[test]
    fn parses_controller_and_agent_identities_exactly() {
        let controller = parse_identity_uri(
            "spiffe://kontainer.internal/swarm/cluster-1/controller/controller-a",
        )
        .unwrap();
        assert_eq!(controller.kind(), WorkloadKind::Controller);
        assert_eq!(controller.cluster_id(), "cluster-1");

        let agent =
            parse_identity_uri("spiffe://kontainer.internal/swarm/cluster-1/node/node-1/agent")
                .unwrap();
        assert_eq!(agent.kind(), WorkloadKind::NodeAgent);
        assert_eq!(agent.node_id(), Some("node-1"));
        assert_eq!(
            agent.canonical_uri(),
            "spiffe://kontainer.internal/swarm/cluster-1/node/node-1/agent"
        );
    }

    #[test]
    fn rejects_non_canonical_or_prefix_confused_identities() {
        for uri in [
            "spiffe://kontainer.internal.attacker/swarm/cluster-1/node/node-1/agent",
            "spiffe://kontainer.internal/swarm/cluster-1/node/node-1/agent/extra",
            "spiffe://kontainer.internal/swarm/cluster-1/node/node%2f1/agent",
            "spiffe://kontainer.internal/swarm/cluster-1/node/node-1/agent?admin=true",
            "spiffe://kontainer.internal/swarm//node/node-1/agent",
        ] {
            assert_eq!(
                parse_identity_uri(uri),
                Err(IdentityError::InvalidWorkloadIdentity)
            );
        }
    }

    #[test]
    fn identity_variants_keep_controller_and_node_fields_distinct() {
        let controller = WorkloadIdentity::Controller {
            cluster_id: "cluster-1".to_owned(),
            instance_id: "controller-a".to_owned(),
        };
        assert_eq!(controller.node_id(), None);
    }
}
