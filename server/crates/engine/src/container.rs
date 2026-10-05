#![forbid(unsafe_code)]

//! Bounded node-local container inventory.
//!
//! The domain owns pagination validation, cursor shape, and safe text
//! handling. Bollard conversion lives in `local.rs` so wire types never
//! escape the adapter.

use chrono::{DateTime, Datelike, FixedOffset};

use crate::{EngineError, EngineErrorKind, EngineOperation};

/// Default page size for container inventory.
pub const DEFAULT_CONTAINER_PAGE_SIZE: usize = 50;
/// Hard maximum page size. Larger requests are rejected before Engine access.
pub const MAX_CONTAINER_PAGE_SIZE: usize = 500;
/// Longest accepted page token (`v1:` plus a full container ID).
pub const MAX_CONTAINER_PAGE_TOKEN_LEN: usize = 67;
/// Longest preserved status text, in bytes, on a UTF-8 boundary.
pub const MAX_CONTAINER_STATUS_LEN: usize = 256;

const PAGE_TOKEN_VERSION: &str = "v1";
const CONTAINER_ID_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerState {
    #[default]
    Unknown,
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainerSummary {
    pub container_id: String,
    pub display_name: String,
    pub image_reference: String,
    pub state: ContainerState,
    pub status: String,
    pub created_at_seconds: i64,
    /// Published ports only, as `host:container/proto`.
    pub published_ports: Vec<String>,
    /// `com.docker.swarm.service.name`, when a Swarm task runs in it.
    pub service_name: Option<String>,
    /// `com.docker.stack.namespace`.
    pub stack_namespace: Option<String>,
    /// `com.docker.swarm.task.id`.
    pub task_id: Option<String>,
    /// Names of the networks it has endpoints on, sorted, at most 16.
    pub networks: Vec<String>,
}

/// Labels that cross the boundary; every other label is dropped in the adapter.
pub const SERVICE_NAME_LABEL: &str = "com.docker.swarm.service.name";
pub const STACK_NAMESPACE_LABEL: &str = "com.docker.stack.namespace";
pub const TASK_ID_LABEL: &str = "com.docker.swarm.task.id";
const MAX_LABEL_VALUE_LEN: usize = 255;

/// A label value kept only when it is short, printable, and non-empty.
#[must_use]
pub fn bounded_label(value: Option<&String>) -> Option<String> {
    value
        .filter(|value| {
            !value.is_empty()
                && value.len() <= MAX_LABEL_VALUE_LEN
                && !value.chars().any(char::is_control)
        })
        .cloned()
}

/// When a container last started and stopped, from inspect. `None` means never.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContainerTimes {
    pub started_at_seconds: Option<i64>,
    pub finished_at_seconds: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Remove,
}

impl ContainerAction {
    pub const ALL: [Self; 6] = [
        Self::Start,
        Self::Stop,
        Self::Restart,
        Self::Pause,
        Self::Unpause,
        Self::Remove,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
            Self::Remove => "remove",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }
}

/// Stop and restart wait this long before killing, unless the caller says otherwise.
pub const DEFAULT_STOP_TIMEOUT_SECONDS: u32 = 10;
pub const MAX_STOP_TIMEOUT_SECONDS: u32 = 600;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContainerActionOptions {
    /// Remove a running container (kill first). Ignored by other actions.
    pub force: bool,
    /// Grace before kill for stop and restart; `None` means the default.
    pub timeout_seconds: Option<u32>,
}

/// Parse Docker's RFC 3339 times (`2026-09-28T18:00:20.814070Z`) to Unix seconds. Docker's
/// "never" (`0001-01-01T00:00:00Z`) and anything unparsable become `None`.
#[must_use]
pub fn parse_docker_time(value: &str) -> Option<i64> {
    parse_docker_instant(value).map(|time| time.timestamp())
}

/// Docker's RFC 3339 time, or `None` for its "never" (any year before 1970) and anything
/// unparsable.
pub(crate) fn parse_docker_instant(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .filter(|time| time.year() >= 1970)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPageRequest {
    page_size: usize,
    cursor_container_id: Option<String>,
}

impl ContainerPageRequest {
    pub fn new(page_size: u32, page_token: &str) -> Result<Self, EngineError> {
        let page_size = if page_size == 0 {
            DEFAULT_CONTAINER_PAGE_SIZE
        } else {
            usize::try_from(page_size).map_err(|_| {
                EngineError::new(
                    EngineErrorKind::InvalidInput,
                    EngineOperation::ListContainers,
                )
            })?
        };
        if page_size == 0 || page_size > MAX_CONTAINER_PAGE_SIZE {
            return Err(EngineError::new(
                EngineErrorKind::InvalidInput,
                EngineOperation::ListContainers,
            ));
        }
        Ok(Self {
            page_size,
            cursor_container_id: parse_page_token(page_token)?,
        })
    }

    #[must_use]
    pub const fn effective_page_size(&self) -> usize {
        self.page_size
    }

    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.cursor_container_id.as_deref()
    }

    #[must_use]
    pub fn daemon_limit(&self) -> i32 {
        // page_size <= 100, so +1 always fits.
        self.page_size as i32 + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPage {
    pub containers: Vec<ContainerSummary>,
    pub next_page_token: Option<String>,
}

/// Parse an opaque page token. Empty means the first page.
pub fn parse_page_token(token: &str) -> Result<Option<String>, EngineError> {
    fn invalid() -> EngineError {
        EngineError::new(
            EngineErrorKind::InvalidInput,
            EngineOperation::ListContainers,
        )
    }

    if token.is_empty() {
        return Ok(None);
    }
    if token.len() > MAX_CONTAINER_PAGE_TOKEN_LEN {
        return Err(invalid());
    }
    let cursor = token
        .strip_prefix(PAGE_TOKEN_VERSION)
        .and_then(|rest| rest.strip_prefix(':'))
        .ok_or_else(invalid)?;
    if !is_valid_container_id(cursor) {
        return Err(invalid());
    }
    Ok(Some(cursor.to_owned()))
}

/// Format the cursor for the page following `container_id`.
#[must_use]
pub fn format_page_token(container_id: &str) -> String {
    format!("{PAGE_TOKEN_VERSION}:{container_id}")
}

#[must_use]
pub fn is_valid_container_id(value: &str) -> bool {
    value.len() == CONTAINER_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Canonical display name: strip the leading Docker slash, prefer a plain
/// name over a legacy link path, and break remaining ties lexically.
#[must_use]
pub fn canonical_display_name(names: &[String]) -> Option<String> {
    let mut candidates: Vec<&str> = names
        .iter()
        .map(|name| name.strip_prefix('/').unwrap_or(name))
        .filter(|name| !name.is_empty())
        .collect();
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_unstable();
    candidates
        .iter()
        .find(|name| !name.contains('/'))
        .or(candidates.first())
        .map(|name| (*name).to_owned())
}

/// Truncate status text to the byte budget on a UTF-8 boundary.
#[must_use]
pub fn truncate_status_text(status: &str) -> String {
    if status.len() <= MAX_CONTAINER_STATUS_LEN {
        return status.to_owned();
    }
    let mut end = MAX_CONTAINER_STATUS_LEN;
    while !status.is_char_boundary(end) {
        end -= 1;
    }
    status[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        ContainerPageRequest, ContainerState, DEFAULT_CONTAINER_PAGE_SIZE, MAX_CONTAINER_PAGE_SIZE,
        MAX_CONTAINER_PAGE_TOKEN_LEN, canonical_display_name, format_page_token,
        is_valid_container_id, parse_page_token, truncate_status_text,
    };
    use crate::{EngineErrorKind, EngineOperation};

    fn valid_id(seed: u8) -> String {
        format!("{:02x}", seed).repeat(32)
    }

    #[test]
    fn page_size_defaults_and_bounds() {
        assert_eq!(
            ContainerPageRequest::new(0, "")
                .unwrap()
                .effective_page_size(),
            DEFAULT_CONTAINER_PAGE_SIZE
        );
        assert_eq!(
            ContainerPageRequest::new(1, "")
                .unwrap()
                .effective_page_size(),
            1
        );
        assert_eq!(
            ContainerPageRequest::new(MAX_CONTAINER_PAGE_SIZE as u32, "")
                .unwrap()
                .effective_page_size(),
            MAX_CONTAINER_PAGE_SIZE
        );
        let error = ContainerPageRequest::new(MAX_CONTAINER_PAGE_SIZE as u32 + 1, "")
            .err()
            .unwrap();
        assert_eq!(error.kind(), EngineErrorKind::InvalidInput);
        assert_eq!(error.operation(), EngineOperation::ListContainers);
    }

    #[test]
    fn page_tokens_round_trip_and_reject() {
        assert_eq!(parse_page_token("").unwrap(), None);
        let id = valid_id(0xab);
        let token = format_page_token(&id);
        assert_eq!(token.len(), MAX_CONTAINER_PAGE_TOKEN_LEN);
        assert_eq!(parse_page_token(&token).unwrap(), Some(id));

        for invalid in [
            "v2:".to_owned() + &valid_id(0x01),
            "v1:short".to_owned(),
            "v1:".to_owned() + &"A".repeat(64),
            "v1:".to_owned() + &"g".repeat(64),
            format!("v1:{}x", valid_id(0x02)),
            "x".repeat(MAX_CONTAINER_PAGE_TOKEN_LEN + 1),
        ] {
            let error = parse_page_token(&invalid).err().unwrap();
            assert_eq!(error.kind(), EngineErrorKind::InvalidInput, "{invalid}");
        }
    }

    #[test]
    fn container_ids_require_full_lowercase_hex() {
        assert!(is_valid_container_id(&valid_id(0x0f)));
        assert!(!is_valid_container_id("abc"));
        assert!(!is_valid_container_id(&"A".repeat(64)));
        assert!(!is_valid_container_id(&valid_id(0x01)[..63]));
    }

    #[test]
    fn display_names_prefer_plain_names_deterministically() {
        assert_eq!(canonical_display_name(&[]), None);
        assert_eq!(
            canonical_display_name(&["/".to_owned(), "".to_owned()]),
            None
        );
        assert_eq!(
            canonical_display_name(&["/web".to_owned()]),
            Some("web".to_owned())
        );
        // Legacy link paths lose to plain names; ties break lexically.
        assert_eq!(
            canonical_display_name(&[
                "/alias/web".to_owned(),
                "/web".to_owned(),
                "/aaa".to_owned(),
            ]),
            Some("aaa".to_owned())
        );
        assert_eq!(
            canonical_display_name(&["/b/a".to_owned(), "/a/b".to_owned()]),
            Some("a/b".to_owned())
        );
    }

    #[test]
    fn status_truncation_respects_utf8_boundaries() {
        assert_eq!(truncate_status_text("Up 3s"), "Up 3s");
        let exact = "e".repeat(256);
        assert_eq!(truncate_status_text(&exact), exact);
        let multibyte = "é".repeat(200);
        let truncated = truncate_status_text(&multibyte);
        assert!(truncated.len() <= 256);
        assert!(multibyte.starts_with(&truncated));
        // Full-width canary from mapping tests stays intact.
        let canary = "Up 2 hours";
        assert_eq!(truncate_status_text(canary), canary);
        assert_eq!(
            ContainerState::Unknown,
            ContainerState::Unknown,
            "state mapping is covered in local.rs adapter tests"
        );
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::{ContainerAction, bounded_label, parse_docker_time};

    #[test]
    fn docker_times_parse_to_unix_seconds_and_never_is_none() {
        assert_eq!(parse_docker_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_docker_time("2026-09-28T18:00:20.814070Z"),
            Some(1_790_618_420)
        );
        assert_eq!(
            parse_docker_time("2026-09-28T20:00:20+02:00"),
            Some(1_790_618_420)
        );
        assert_eq!(
            parse_docker_time("2024-02-29T12:00:00Z"),
            Some(1_709_208_000)
        );
        for never in [
            "0001-01-01T00:00:00Z",
            "",
            "not a time",
            "2026-13-01T00:00:00Z",
        ] {
            assert_eq!(parse_docker_time(never), None, "{never}");
        }
    }

    #[test]
    fn actions_round_trip_and_labels_are_bounded() {
        for action in ContainerAction::ALL {
            assert_eq!(ContainerAction::parse(action.as_str()), Some(action));
        }
        assert_eq!(ContainerAction::parse("kill"), None);
        assert_eq!(
            bounded_label(Some(&"web".to_owned())),
            Some("web".to_owned())
        );
        assert_eq!(bounded_label(Some(&String::new())), None);
        assert_eq!(bounded_label(Some(&"a\nb".to_owned())), None);
        assert_eq!(bounded_label(Some(&"x".repeat(256))), None);
        assert_eq!(bounded_label(None), None);
    }
}
