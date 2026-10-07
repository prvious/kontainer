//! A Swarm service's logs as typed lines, gathered by the manager from every task.

use std::pin::Pin;

use futures_util::Stream;

use crate::{EngineError, container::parse_docker_instant};

/// What to read: history since a time and at most `tail` lines of it, then optionally follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceLogRequest {
    pub since_unix_ms: Option<i64>,
    pub tail: u32,
    pub follow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Stdout,
    Stderr,
}

/// One line with the task and node that wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceLogLine {
    pub source: LogSource,
    pub timestamp_unix_ms: i64,
    pub node_id: String,
    pub task_id: String,
    /// The message without its trailing newline; invalid UTF-8 is replaced.
    pub text: String,
}

pub type ServiceLogStream = Pin<Box<dyn Stream<Item = Result<ServiceLogLine, EngineError>> + Send>>;

const NODE_ID_KEY: &str = "com.docker.swarm.node.id";
const TASK_ID_KEY: &str = "com.docker.swarm.task.id";

/// `<RFC 3339 time> <k=v,…> <message>` as Docker writes service logs with `timestamps` and
/// `details`. The details are optional; a line without a parseable time is dropped.
#[must_use]
pub fn parse_service_log_line(source: LogSource, bytes: &[u8]) -> Option<ServiceLogLine> {
    let raw = String::from_utf8_lossy(bytes);
    let raw = raw.strip_suffix('\n').unwrap_or(&raw);
    let raw = raw.strip_suffix('\r').unwrap_or(raw);
    let (timestamp, rest) = raw.split_once(' ').unwrap_or((raw, ""));
    let timestamp_unix_ms = parse_docker_instant(timestamp)?.timestamp_millis();
    let (details, text) = match rest.split_once(' ') {
        Some((first, text)) if is_details(first) => (first, text),
        None if is_details(rest) => (rest, ""),
        _ => ("", rest),
    };
    let mut node_id = String::new();
    let mut task_id = String::new();
    for pair in details.split(',') {
        match pair.split_once('=') {
            Some((NODE_ID_KEY, value)) => value.clone_into(&mut node_id),
            Some((TASK_ID_KEY, value)) => value.clone_into(&mut task_id),
            _ => {}
        }
    }
    Some(ServiceLogLine {
        source,
        timestamp_unix_ms,
        node_id,
        task_id,
        text: text.to_owned(),
    })
}

fn is_details(token: &str) -> bool {
    token.starts_with("com.docker.swarm.") && token.contains('=')
}

#[cfg(test)]
mod tests {
    use super::{LogSource, ServiceLogLine, parse_service_log_line};

    #[test]
    fn details_give_the_task_and_node_and_the_time_keeps_milliseconds() {
        let line = parse_service_log_line(
            LogSource::Stderr,
            b"2026-09-28T16:17:54.397779085Z com.docker.swarm.node.id=tj42rq5nzwf8r8avruy1a45b6,com.docker.swarm.service.id=eb6ymskrt4ij70dco3menddxl,com.docker.swarm.task.id=x0xza695s4xgf5rqmbvkpsizf 2026/09/28 16:17:54 Starting up on port 80\n",
        )
        .unwrap();
        assert_eq!(
            line,
            ServiceLogLine {
                source: LogSource::Stderr,
                timestamp_unix_ms: 1_790_612_274_397,
                node_id: "tj42rq5nzwf8r8avruy1a45b6".to_owned(),
                task_id: "x0xza695s4xgf5rqmbvkpsizf".to_owned(),
                text: "2026/09/28 16:17:54 Starting up on port 80".to_owned(),
            }
        );
        let offset =
            parse_service_log_line(LogSource::Stdout, b"2026-09-28T18:17:54.397779085+02:00 x")
                .unwrap();
        assert_eq!(offset.timestamp_unix_ms, line.timestamp_unix_ms);
    }

    #[test]
    fn lines_without_details_or_text_still_parse_and_garbage_is_dropped() {
        let bare =
            parse_service_log_line(LogSource::Stdout, b"2026-09-28T16:17:54Z hello world").unwrap();
        assert_eq!(
            (bare.node_id.as_str(), bare.text.as_str()),
            ("", "hello world")
        );
        assert_eq!(bare.timestamp_unix_ms % 1000, 0);
        let empty = parse_service_log_line(
            LogSource::Stdout,
            b"2026-09-28T16:17:54.5Z com.docker.swarm.task.id=abc\n",
        )
        .unwrap();
        assert_eq!((empty.task_id.as_str(), empty.text.as_str()), ("abc", ""));
        assert_eq!(empty.timestamp_unix_ms % 1000, 500);
        let invalid =
            parse_service_log_line(LogSource::Stdout, b"2026-09-28T16:17:54Z \xff\xfe").unwrap();
        assert_eq!(invalid.text, "\u{fffd}\u{fffd}");
        assert_eq!(
            parse_service_log_line(LogSource::Stdout, b"no time here"),
            None
        );
    }
}
