//! Log streaming rules in the controller's own terms: limits, validation, and truncation.

use std::time::Duration;

use kontainer_engine::{ServiceLogRequest, is_valid_swarm_id};
use kontainer_stream_protocol::{DEFAULT_LOG_TAIL, LogsOpen, MAX_LOG_TAIL};

/// Open log connections one access session may hold; the next is refused `limit_reached`.
pub const MAX_LOG_CONNECTIONS_PER_SESSION: usize = 8;
/// A line has to fit one 64 KiB frame with its ids, so longer ones are cut here and flagged.
pub const MAX_LINE_BYTES: usize = 60 * 1024;
/// Lines held for a client that has fallen behind before the oldest are dropped.
pub const BACKLOG_LINES: usize = 2_000;
/// How often a following stream checks that the manager still answers.
pub const LIVENESS_INTERVAL: Duration = Duration::from_secs(5);

/// Why an open was refused before anything was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenRejection {
    InvalidRequest,
}

/// A valid open as the engine asks for it: always following.
pub fn validate_open(open: &LogsOpen) -> Result<ServiceLogRequest, OpenRejection> {
    let tail = open.tail.unwrap_or(DEFAULT_LOG_TAIL);
    if !is_valid_swarm_id(&open.service_id)
        || tail > MAX_LOG_TAIL
        || open.since_unix_ms.is_some_and(|since| since < 0)
    {
        return Err(OpenRejection::InvalidRequest);
    }
    Ok(ServiceLogRequest {
        since_unix_ms: open.since_unix_ms,
        tail,
        follow: true,
    })
}

/// At most [`MAX_LINE_BYTES`], cut on a character boundary; `true` when something was cut.
#[must_use]
pub fn truncate_line(mut text: String) -> (String, bool) {
    if text.len() <= MAX_LINE_BYTES {
        return (text, false);
    }
    let mut end = MAX_LINE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    (text, true)
}

#[cfg(test)]
mod tests {
    use kontainer_stream_protocol::LogsOpen;
    use uuid::Uuid;

    use super::{
        MAX_LINE_BYTES, MAX_LOG_CONNECTIONS_PER_SESSION, OpenRejection, truncate_line,
        validate_open,
    };
    use crate::stream_slots::SessionSlots;

    const SERVICE: &str = "eb6ymskrt4ij70dco3menddxl";

    #[test]
    fn opens_default_the_tail_follow_and_refuse_bad_ids_tails_and_times() {
        let open = |service: &str, since, tail| LogsOpen {
            service_id: service.to_owned(),
            since_unix_ms: since,
            tail,
        };
        let request = validate_open(&open(SERVICE, None, None)).unwrap();
        assert_eq!((request.tail, request.follow), (500, true));
        assert_eq!(
            validate_open(&open(SERVICE, Some(1), Some(5_000)))
                .unwrap()
                .tail,
            5_000
        );
        for bad in [
            open("webapp_web", None, None),
            open(SERVICE, None, Some(5_001)),
            open(SERVICE, Some(-1), None),
        ] {
            assert_eq!(
                validate_open(&bad),
                Err(OpenRejection::InvalidRequest),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn long_lines_are_cut_on_a_character_boundary_and_flagged() {
        assert_eq!(
            truncate_line("short".to_owned()),
            ("short".to_owned(), false)
        );
        let (cut, truncated) = truncate_line("é".repeat(MAX_LINE_BYTES));
        assert!(truncated);
        assert!(cut.len() <= MAX_LINE_BYTES && cut.len() > MAX_LINE_BYTES - 2);
        assert!(cut.chars().all(|character| character == 'é'));
    }

    #[test]
    fn a_session_holds_eight_connections_and_frees_them_as_they_end() {
        let connections = SessionSlots::new(MAX_LOG_CONNECTIONS_PER_SESSION);
        let family = Uuid::now_v7();
        let guards: Vec<_> = (0..MAX_LOG_CONNECTIONS_PER_SESSION)
            .map(|_| connections.try_acquire(family).unwrap())
            .collect();
        assert!(
            connections.try_acquire(family).is_none(),
            "the ninth is refused"
        );
        assert!(
            connections.try_acquire(Uuid::now_v7()).is_some(),
            "another session is not affected"
        );
        drop(guards);
        assert_eq!(connections.open(family), 0);
        assert!(connections.try_acquire(family).is_some());
    }
}
