use std::{fmt, num::ParseIntError, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineAccessScope {
    Manager,
    NodeLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EngineApiVersion {
    pub major: u16,
    pub minor: u16,
}

impl EngineApiVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

impl fmt::Display for EngineApiVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for EngineApiVersion {
    type Err = EngineApiVersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (major, minor) = value
            .split_once('.')
            .ok_or(EngineApiVersionParseError::InvalidFormat)?;

        if minor.contains('.') || major.is_empty() || minor.is_empty() {
            return Err(EngineApiVersionParseError::InvalidFormat);
        }

        Ok(Self {
            major: major.parse()?,
            minor: minor.parse()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineApiVersionParseError {
    InvalidFormat,
    InvalidNumber,
}

impl From<ParseIntError> for EngineApiVersionParseError {
    fn from(_error: ParseIntError) -> Self {
        Self::InvalidNumber
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineCapability {
    LocalContainerOperations,
    SwarmNodeOperations,
    SwarmManagerOperations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityStatus {
    Supported,
    Unsupported(UnsupportedCapabilityReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedCapabilityReason {
    SwarmStateUnavailable,
    SwarmInactive,
    SwarmPending,
    SwarmError,
    SwarmLocked,
    ManagerControlUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCapabilities {
    pub local_container_operations: CapabilityStatus,
    pub swarm_node_operations: CapabilityStatus,
    pub swarm_manager_operations: CapabilityStatus,
}

impl EngineCapabilities {
    #[must_use]
    pub const fn status(&self, capability: EngineCapability) -> CapabilityStatus {
        match capability {
            EngineCapability::LocalContainerOperations => self.local_container_operations,
            EngineCapability::SwarmNodeOperations => self.swarm_node_operations,
            EngineCapability::SwarmManagerOperations => self.swarm_manager_operations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDescription {
    pub access_scope: EngineAccessScope,
    pub daemon_version: String,
    pub daemon_api_version: EngineApiVersion,
    pub minimum_api_version: EngineApiVersion,
    pub negotiated_api_version: EngineApiVersion,
    pub operating_system: String,
    pub architecture: String,
    pub capabilities: EngineCapabilities,
}

#[cfg(test)]
mod tests {
    use super::{
        CapabilityStatus, EngineApiVersion, EngineApiVersionParseError, EngineCapabilities,
        EngineCapability, UnsupportedCapabilityReason,
    };

    #[test]
    fn api_versions_are_parsed_and_ordered_numerically() {
        let older: EngineApiVersion = "1.47".parse().unwrap();
        let newer: EngineApiVersion = "1.53".parse().unwrap();

        assert_eq!(older, EngineApiVersion::new(1, 47));
        assert!(older < newer);
        assert_eq!(newer.to_string(), "1.53");
    }

    #[test]
    fn malformed_api_versions_are_rejected() {
        assert_eq!(
            "1".parse::<EngineApiVersion>(),
            Err(EngineApiVersionParseError::InvalidFormat)
        );
        assert_eq!(
            "1.latest".parse::<EngineApiVersion>(),
            Err(EngineApiVersionParseError::InvalidNumber)
        );
        assert_eq!(
            "1.47.2".parse::<EngineApiVersion>(),
            Err(EngineApiVersionParseError::InvalidFormat)
        );
    }

    #[test]
    fn unsupported_capabilities_keep_a_typed_reason() {
        let capabilities = EngineCapabilities {
            local_container_operations: CapabilityStatus::Supported,
            swarm_node_operations: CapabilityStatus::Unsupported(
                UnsupportedCapabilityReason::SwarmInactive,
            ),
            swarm_manager_operations: CapabilityStatus::Unsupported(
                UnsupportedCapabilityReason::SwarmInactive,
            ),
        };

        assert_eq!(
            capabilities.status(EngineCapability::SwarmManagerOperations),
            CapabilityStatus::Unsupported(UnsupportedCapabilityReason::SwarmInactive)
        );
    }
}
