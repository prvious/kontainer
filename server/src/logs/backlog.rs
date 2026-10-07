//! Lines waiting for room in the transport's bounded queue. When the client falls behind, the
//! oldest waiting lines are dropped and counted, and the count goes out as one `LogGap` ahead of
//! the lines that survived.

use std::collections::VecDeque;

use kontainer_stream_protocol::{LogGap, LogLine, LogMessage};
use kontainer_stream_transport::OutboundFrameError;

pub struct Backlog {
    lines: VecDeque<LogLine>,
    capacity: usize,
    dropped: u64,
}

/// What a flush left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flush {
    Drained,
    /// The transport's queue is full; the rest waits.
    Blocked,
}

impl Backlog {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity.min(256)),
            capacity: capacity.max(1),
            dropped: 0,
        }
    }

    pub fn push(&mut self, line: LogLine) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.lines.push_back(line);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.dropped == 0
    }

    /// Send the pending gap, then lines, until the queue is full or nothing is left. Only a
    /// closed or broken session is an error.
    pub fn flush(
        &mut self,
        mut send: impl FnMut(LogMessage) -> Result<(), OutboundFrameError>,
    ) -> Result<Flush, OutboundFrameError> {
        if self.dropped > 0 {
            match send(LogMessage::Gap(LogGap {
                dropped: self.dropped,
            })) {
                Ok(()) => self.dropped = 0,
                Err(OutboundFrameError::QueueOverloaded) => return Ok(Flush::Blocked),
                Err(error) => return Err(error),
            }
        }
        while let Some(line) = self.lines.front() {
            match send(LogMessage::Line(line.clone())) {
                Ok(()) => {
                    self.lines.pop_front();
                }
                Err(OutboundFrameError::QueueOverloaded) => return Ok(Flush::Blocked),
                Err(error) => return Err(error),
            }
        }
        Ok(Flush::Drained)
    }
}

#[cfg(test)]
mod tests {
    use kontainer_stream_protocol::{LogGap, LogLine, LogMessage, LogStreamKind};
    use kontainer_stream_transport::OutboundFrameError;

    use super::{Backlog, Flush};

    fn line(seq: u64) -> LogLine {
        LogLine {
            ts_unix_ms: 1_790_000_000_000 + i64::try_from(seq).unwrap(),
            stream: LogStreamKind::Stdout,
            task_id: String::new(),
            node_id: String::new(),
            line: format!("line {seq}"),
            truncated: false,
        }
    }

    #[test]
    fn a_full_backlog_drops_the_oldest_and_reports_one_gap_ahead_of_the_survivors() {
        let mut backlog = Backlog::new(3);
        for seq in 1..=8 {
            backlog.push(line(seq));
            // The client is not reading at all.
            assert_eq!(
                backlog.flush(|_| Err(OutboundFrameError::QueueOverloaded)),
                Ok(Flush::Blocked)
            );
        }
        let mut room = 10;
        let mut sent = Vec::new();
        let flushed = backlog.flush(|message| {
            if room == 0 {
                return Err(OutboundFrameError::QueueOverloaded);
            }
            room -= 1;
            sent.push(message);
            Ok(())
        });
        assert_eq!(flushed, Ok(Flush::Drained));
        assert_eq!(
            sent,
            vec![
                LogMessage::Gap(LogGap { dropped: 5 }),
                LogMessage::Line(line(6)),
                LogMessage::Line(line(7)),
                LogMessage::Line(line(8)),
            ]
        );
        assert!(backlog.is_empty());
    }

    #[test]
    fn a_partly_open_queue_keeps_order_and_a_closed_session_is_an_error() {
        let mut backlog = Backlog::new(10);
        for seq in 1..=3 {
            backlog.push(line(seq));
        }
        let mut sent = Vec::new();
        let flushed = backlog.flush(|message| {
            if sent.len() == 2 {
                return Err(OutboundFrameError::QueueOverloaded);
            }
            sent.push(message);
            Ok(())
        });
        assert_eq!(flushed, Ok(Flush::Blocked));
        assert_eq!(sent.len(), 2);
        assert!(!backlog.is_empty());
        assert_eq!(
            backlog.flush(|_| Err(OutboundFrameError::SessionClosed)),
            Err(OutboundFrameError::SessionClosed)
        );
    }
}
