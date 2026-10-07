//! Stacks derived from service labels: `GET /api/v1/stacks`, `GET /api/v1/stacks/{name}/plan`, and
//! `DELETE /api/v1/stacks/{name}`.

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::client::{
    ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json, parse_timestamp, valid_display_name,
};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::{AdmittedOperation, IdempotencyKey};

const MAX_STACKS: usize = 1_000;
const MAX_PLAN_ENTRIES: usize = 2_000;

/// A stack name: `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, 1–128 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StackName(String);

impl StackName {
    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        let bytes = value.as_bytes();
        let valid = (1..=128).contains(&bytes.len())
            && bytes[0].is_ascii_alphanumeric()
            && bytes[1..]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'));
        if valid {
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
pub enum StackStatus {
    Running,
    Partial,
    Stopped,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackItem {
    pub name: String,
    pub status: StackStatus,
    pub services: u32,
    pub running_containers: u64,
    pub desired_containers: u64,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub last_deployed: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackList {
    pub items: Vec<StackItem>,
}

/// What removing a stack takes away, and the volumes it leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackRemovalPlan {
    pub services: Vec<String>,
    pub networks: Vec<String>,
    pub volumes_retained: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StackListBody {
    items: Vec<StackItemBody>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StackItemBody {
    name: String,
    status: StackStatus,
    services: u32,
    running_containers: u64,
    desired_containers: u64,
    cpu_percent: Option<f64>,
    memory_bytes: Option<u64>,
    last_deployed: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StackPlanBody {
    services: Vec<String>,
    networks: Vec<String>,
    volumes_retained: Vec<String>,
}

impl StackListBody {
    pub(crate) fn into_list(self) -> Result<StackList, NativeClientError> {
        if self.items.len() > MAX_STACKS {
            return Err(NativeClientError::InvalidResponse);
        }
        let items = self
            .items
            .into_iter()
            .map(|item| {
                let cpu_ok = item
                    .cpu_percent
                    .is_none_or(|cpu| cpu.is_finite() && cpu >= 0.0);
                if !valid_display_name(&item.name) || !cpu_ok {
                    return Err(NativeClientError::InvalidResponse);
                }
                Ok(StackItem {
                    last_deployed: item
                        .last_deployed
                        .as_deref()
                        .map(parse_timestamp)
                        .transpose()?,
                    name: item.name,
                    status: item.status,
                    services: item.services,
                    running_containers: item.running_containers,
                    desired_containers: item.desired_containers,
                    cpu_percent: item.cpu_percent,
                    memory_bytes: item.memory_bytes,
                })
            })
            .collect::<Result<_, NativeClientError>>()?;
        Ok(StackList { items })
    }
}

impl StackPlanBody {
    fn into_plan(self) -> Result<StackRemovalPlan, NativeClientError> {
        let bounded = |values: &[String]| {
            values.len() <= MAX_PLAN_ENTRIES && values.iter().all(|value| value.len() <= 512)
        };
        if !bounded(&self.services) || !bounded(&self.networks) || !bounded(&self.volumes_retained)
        {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(StackRemovalPlan {
            services: self.services,
            networks: self.networks,
            volumes_retained: self.volumes_retained,
        })
    }
}

impl NativeClient {
    /// Every stack with its task counts and container figures.
    pub async fn list_stacks(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<StackList, NativeClientError> {
        let url = self.endpoint("/api/v1/stacks")?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute_with_error_policy(
                request,
                RetryPolicy::Once,
                self.limits.collection_body_limit,
                cancellation,
                ErrorBodyPolicy::Inventory,
            )
            .await?;
        decode_json::<StackListBody>(&body)?.into_list()
    }

    /// What removing `name` takes away and retains.
    pub async fn stack_removal_plan(
        &self,
        access: &AccessCredential,
        name: &StackName,
        cancellation: &CancellationToken,
    ) -> Result<StackRemovalPlan, NativeClientError> {
        let url = self.endpoint(&format!("/api/v1/stacks/{}/plan", name.as_str()))?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute_with_error_policy(
                request,
                RetryPolicy::Once,
                self.limits.collection_body_limit,
                cancellation,
                ErrorBodyPolicy::Inventory,
            )
            .await?;
        decode_json::<StackPlanBody>(&body)?.into_plan()
    }

    /// Admit one `stack.remove` operation.
    pub async fn remove_stack(
        &self,
        access: &AccessCredential,
        name: &StackName,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let path = format!("/api/v1/stacks/{}", name.as_str());
        self.admit_with(
            Method::DELETE,
            access,
            &path,
            key,
            &serde_json::json!({}),
            cancellation,
        )
        .await
    }
}
