//! Swarm services: the list with running counts, one service's newest task error, and scale /
//! restart / rollback as admitted operations. A manager that can't answer is
//! [`NativeClientError::QuorumUnavailable`](crate::NativeClientError::QuorumUnavailable).

mod calls;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::{parse_timestamp, valid_display_name};
use crate::error::NativeClientError;

/// The most services one list may carry.
pub const MAX_SERVICES: usize = 2_000;
/// Replicas one scale may ask for.
pub const MAX_REPLICAS: u64 = 1_000;

/// A Swarm service id: 25 lowercase alphanumerics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceId(String);

impl ServiceId {
    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        if value.len() == 25
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
        {
            Ok(Self(value.to_owned()))
        } else {
            Err(NativeClientError::InvalidRequest)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceMode {
    Replicated,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceItem {
    pub id: String,
    pub name: String,
    /// Without the digest.
    pub image: String,
    pub image_reference: String,
    pub mode: ServiceMode,
    pub desired_replicas: u64,
    pub running_replicas: u64,
    pub ports: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub version: u64,
    pub stack: Option<String>,
    pub update_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceList {
    pub items: Vec<ServiceItem>,
    pub snapshot_at: DateTime<Utc>,
}

/// What the replica tooltip asks of one service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDetail {
    pub latest_error: Option<String>,
}

/// What a service operation asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    Scale { replicas: u64 },
    Restart,
    Rollback,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ServiceListBody {
    items: Vec<ServiceItemBody>,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ServiceItemBody {
    id: String,
    name: String,
    image: String,
    image_reference: String,
    mode: ServiceMode,
    desired_replicas: u64,
    running_replicas: u64,
    ports: Vec<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    version: u64,
    stack: Option<String>,
    update_state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ServiceDetailBody {
    latest_error: Option<String>,
}

fn optional_time(value: Option<&str>) -> Result<Option<DateTime<Utc>>, NativeClientError> {
    value.map(parse_timestamp).transpose()
}

impl ServiceItemBody {
    fn into_item(self) -> Result<ServiceItem, NativeClientError> {
        let bounded = |value: &str| value.len() <= 512;
        if ServiceId::parse(&self.id).is_err()
            || !valid_display_name(&self.name)
            || !bounded(&self.image_reference)
            || self.ports.len() > 64
        {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(ServiceItem {
            created_at: optional_time(self.created_at.as_deref())?,
            updated_at: optional_time(self.updated_at.as_deref())?,
            id: self.id,
            name: self.name,
            image: self.image,
            image_reference: self.image_reference,
            mode: self.mode,
            desired_replicas: self.desired_replicas,
            running_replicas: self.running_replicas,
            ports: self.ports,
            version: self.version,
            stack: self.stack,
            update_state: self.update_state,
        })
    }
}

impl ServiceListBody {
    pub(crate) fn into_list(self) -> Result<ServiceList, NativeClientError> {
        if self.items.len() > MAX_SERVICES {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(ServiceList {
            items: self
                .items
                .into_iter()
                .map(ServiceItemBody::into_item)
                .collect::<Result<_, _>>()?,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}

impl ServiceDetailBody {
    pub(crate) fn into_detail(self) -> ServiceDetail {
        ServiceDetail {
            latest_error: self.latest_error,
        }
    }
}
