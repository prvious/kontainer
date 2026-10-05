//! Networks in Kontainer's own terms: node-local ones on an agent, swarm-scope ones on a manager.

use std::collections::BTreeMap;

/// Networks Docker creates itself; they can never be removed.
pub const BUILTIN_NETWORKS: [&str; 5] = ["host", "bridge", "none", "docker_gwbridge", "ingress"];
/// Longest network name the controller accepts.
pub const MAX_NETWORK_NAME_LEN: usize = 128;
/// Container names listed for one network.
pub const MAX_NETWORK_ENDPOINTS: usize = 256;

/// One network as the daemon sees it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkSummary {
    /// 64 lowercase hex characters (swarm networks use 25-character ids).
    pub id: String,
    pub name: String,
    pub driver: String,
    /// `local`, `swarm`, or `global`.
    pub scope: String,
    /// IPAM subnets in CIDR form.
    pub subnets: Vec<String>,
    /// Running containers on this daemon with an endpoint on it.
    pub containers: u32,
    pub created_at_seconds: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub attachable: bool,
}

impl NetworkSummary {
    #[must_use]
    pub fn builtin(&self) -> bool {
        is_builtin_network(&self.name)
    }
}

/// One container endpoint on a network, on this daemon.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkEndpoint {
    pub container_id: String,
    pub name: String,
}

/// One network with the endpoints this daemon has on it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkDetail {
    pub network: NetworkSummary,
    pub endpoints: Vec<NetworkEndpoint>,
}

/// What `create_network` makes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkCreate {
    pub name: String,
    /// `overlay` on a manager, `bridge` on a node.
    pub driver: String,
    pub attachable: bool,
    pub subnet: Option<String>,
    pub labels: BTreeMap<String, String>,
}

#[must_use]
pub fn is_builtin_network(name: &str) -> bool {
    BUILTIN_NETWORKS.contains(&name)
}

/// Network names the controller creates: `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, 1–128 characters.
#[must_use]
pub fn is_valid_network_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=MAX_NETWORK_NAME_LEN).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

/// A network id as Docker prints it: 25 (swarm) or 64 (local) lowercase alphanumerics.
#[must_use]
pub fn is_valid_network_id(value: &str) -> bool {
    matches!(value.len(), 25 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
}

/// A canonical CIDR: an IPv4 or IPv6 network address with its prefix and no host bits set.
#[must_use]
pub fn is_valid_cidr(value: &str) -> bool {
    use std::net::IpAddr;

    let Some((address, raw_prefix)) = value.split_once('/') else {
        return false;
    };
    let Ok(prefix) = raw_prefix.parse::<u32>() else {
        return false;
    };
    if prefix.to_string() != raw_prefix {
        return false;
    }
    match address.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) if (8..=32).contains(&prefix) && ip.to_string() == address => {
            let bits = u32::from(ip);
            bits & (u32::MAX.checked_shr(prefix).unwrap_or(0)) == 0
        }
        Ok(IpAddr::V6(ip)) if (8..=128).contains(&prefix) => {
            let bits = u128::from(ip);
            bits & (u128::MAX.checked_shr(prefix).unwrap_or(0)) == 0
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_builtin_network, is_valid_cidr, is_valid_network_id, is_valid_network_name};

    #[test]
    fn names_ids_and_builtins() {
        for name in ["app-network", "a", "toolbox_kontainer", "v1.2"] {
            assert!(is_valid_network_name(name), "{name}");
        }
        for name in ["", "-net", "has space", "sl/ash", &"a".repeat(129)] {
            assert!(!is_valid_network_name(name), "{name}");
        }
        assert!(is_valid_network_id(&"a".repeat(64)));
        assert!(is_valid_network_id("x7kq2m9p0r1s3t5u7w9y0z2b4"));
        assert!(!is_valid_network_id(&"A".repeat(64)));
        assert!(!is_valid_network_id("short"));
        for builtin in ["host", "bridge", "none", "docker_gwbridge", "ingress"] {
            assert!(is_builtin_network(builtin));
        }
        assert!(!is_builtin_network("toolbox_kontainer"));
    }

    #[test]
    fn subnets_are_canonical_cidrs() {
        for cidr in [
            "10.0.9.0/24",
            "172.28.0.0/16",
            "10.0.0.0/8",
            "192.168.1.128/25",
            "fd00:1::/64",
        ] {
            assert!(is_valid_cidr(cidr), "{cidr}");
        }
        for cidr in [
            "10.0.9.1/24",
            "10.0.9.0",
            "10.0.9.0/33",
            "10.0.9.0/4",
            "10.0.9/24",
            "999.0.0.0/8",
            "10.0.9.0/024",
            "fd00::1/64",
            "banana",
        ] {
            assert!(!is_valid_cidr(cidr), "{cidr}");
        }
    }
}
