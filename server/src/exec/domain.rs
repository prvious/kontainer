//! Exec rules in the controller's own terms: limits, timeouts, and what an open may ask for.

use std::time::Duration;

use kontainer_engine::is_valid_container_id;
use kontainer_stream_protocol::{ExecOpen, MAX_EXEC_DIMENSION};

use crate::rpc::AgentExecOpen;

/// Exec sessions one access session may hold; the next is refused `limit_reached`.
pub const MAX_EXEC_SESSIONS_PER_SESSION: usize = 4;
/// Output frames carry at most this much, well inside the 64 KiB frame limit.
pub const OUTPUT_CHUNK: usize = 32 * 1024;
const MAX_SHELL_LEN: usize = 256;
const MAX_NODE_ID_LEN: usize = 128;

/// How long a session may sit idle, last in all, and wait for a client that stopped reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecTiming {
    pub idle: Duration,
    pub max: Duration,
    pub stall: Duration,
}

impl Default for ExecTiming {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(30 * 60),
            max: Duration::from_secs(8 * 60 * 60),
            stall: Duration::from_secs(10),
        }
    }
}

/// Why a session ended, as `ExecExit.reason` says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    Exited,
    IdleTimeout,
    MaxDuration,
    ConnectionLost,
}

impl ExitReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::IdleTimeout => "idle_timeout",
            Self::MaxDuration => "max_duration",
            Self::ConnectionLost => "connection_lost",
        }
    }
}

fn valid_node_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NODE_ID_LEN
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

#[must_use]
pub fn valid_dimensions(cols: u32, rows: u32) -> bool {
    (1..=MAX_EXEC_DIMENSION).contains(&cols) && (1..=MAX_EXEC_DIMENSION).contains(&rows)
}

/// The node to ask and what to ask it, or `None` when the open can't be tried.
#[must_use]
pub fn validate_open(open: &ExecOpen) -> Option<(String, AgentExecOpen)> {
    let shell_ok = open.shell.as_deref().is_none_or(|shell| {
        !shell.is_empty() && shell.len() <= MAX_SHELL_LEN && !shell.chars().any(char::is_control)
    });
    (valid_node_id(&open.node_id)
        && is_valid_container_id(&open.container_id)
        && shell_ok
        && valid_dimensions(open.cols, open.rows))
    .then(|| {
        (
            open.node_id.clone(),
            AgentExecOpen {
                container_id: open.container_id.clone(),
                shell: open.shell.clone(),
                cols: open.cols,
                rows: open.rows,
            },
        )
    })
}

/// Agent error codes as the desktop sees them: an engine failure on the node is the node's.
#[must_use]
pub fn client_error_code(agent_code: &str) -> &str {
    match agent_code {
        "target_not_running" | "shell_not_found" | "invalid_request" => agent_code,
        _ => "node_unavailable",
    }
}

#[cfg(test)]
mod tests {
    use kontainer_stream_protocol::ExecOpen;

    use super::{client_error_code, validate_open};

    fn open(node: &str, container: &str, shell: Option<&str>, cols: u32, rows: u32) -> ExecOpen {
        ExecOpen {
            node_id: node.to_owned(),
            container_id: container.to_owned(),
            shell: shell.map(str::to_owned),
            cols,
            rows,
        }
    }

    #[test]
    fn opens_name_a_node_a_container_a_sane_shell_and_a_real_size() {
        let container = "c".repeat(64);
        let (node, agent) =
            validate_open(&open("worker-1", &container, Some("/bin/zsh"), 80, 24)).unwrap();
        assert_eq!(
            (node.as_str(), agent.shell.as_deref(), agent.cols),
            ("worker-1", Some("/bin/zsh"), 80)
        );
        for bad in [
            open("..", &container, None, 80, 24),
            open("bad/node", &container, None, 80, 24),
            open("worker-1", "web", None, 80, 24),
            open("worker-1", &container, Some(""), 80, 24),
            open("worker-1", &container, Some("sh\u{7}"), 80, 24),
            open("worker-1", &container, None, 0, 24),
            open("worker-1", &container, None, 80, 1_001),
        ] {
            assert!(validate_open(&bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn engine_failures_on_the_node_read_as_node_unavailable() {
        assert_eq!(
            client_error_code("target_not_running"),
            "target_not_running"
        );
        assert_eq!(client_error_code("shell_not_found"), "shell_not_found");
        assert_eq!(client_error_code("engine_error"), "node_unavailable");
        assert_eq!(client_error_code("anything else"), "node_unavailable");
    }
}
