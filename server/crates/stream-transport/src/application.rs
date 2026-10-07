use std::sync::Arc;

use async_trait::async_trait;
use kontainer_stream_protocol::StreamFrame;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};

use crate::{StreamClose, StreamLimits};

#[async_trait]
pub trait StreamApplication: Send + Sync + 'static {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError>;
}

#[derive(Clone)]
pub struct OutboundSender {
    sender: mpsc::Sender<QueuedFrame>,
    byte_budget: Arc<Semaphore>,
    maximum_frame_bytes: usize,
    signal: watch::Sender<SessionSignal>,
}

/// What the session writer does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionSignal {
    Running,
    Close(StreamClose),
    /// Close after the frames already queued are written; the application asked for it.
    Finish(StreamClose),
    PeerClosed,
}

impl OutboundSender {
    /// Resolves once the session's writer has stopped, so a producer can stop too.
    pub async fn closed(&self) {
        self.sender.closed().await;
    }

    /// Close the session with `close` once the writer has drained the queue (within the shutdown
    /// grace); a close already under way wins. This doesn't refuse later frames: one sent before
    /// the drain finishes is still written, and `try_send` fails with
    /// [`OutboundFrameError::SessionClosed`] only after the writer stops.
    pub fn close(&self, close: StreamClose) {
        self.signal.send_if_modified(|current| {
            if *current == SessionSignal::Running {
                *current = SessionSignal::Finish(close);
                true
            } else {
                false
            }
        });
    }

    pub fn try_send(&self, frame: StreamFrame) -> Result<(), OutboundFrameError> {
        let encoded = frame.encode();
        if encoded.len() > self.maximum_frame_bytes {
            return Err(OutboundFrameError::FrameTooLarge);
        }
        let byte_count =
            u32::try_from(encoded.len()).map_err(|_| OutboundFrameError::FrameTooLarge)?;
        let byte_permit = self
            .byte_budget
            .clone()
            .try_acquire_many_owned(byte_count)
            .map_err(|_| OutboundFrameError::QueueOverloaded)?;

        self.sender
            .try_send(QueuedFrame {
                encoded,
                _byte_permit: byte_permit,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => OutboundFrameError::QueueOverloaded,
                mpsc::error::TrySendError::Closed(_) => OutboundFrameError::SessionClosed,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboundFrameError {
    FrameTooLarge,
    QueueOverloaded,
    SessionClosed,
}

pub(crate) struct QueuedFrame {
    pub(crate) encoded: Vec<u8>,
    _byte_permit: OwnedSemaphorePermit,
}

pub(crate) fn outbound_queue(
    limits: StreamLimits,
    signal: watch::Sender<SessionSignal>,
) -> (OutboundSender, mpsc::Receiver<QueuedFrame>) {
    let (sender, receiver) = mpsc::channel(limits.maximum_queued_frames());
    (
        OutboundSender {
            sender,
            byte_budget: Arc::new(Semaphore::new(limits.maximum_queued_bytes())),
            maximum_frame_bytes: limits.maximum_frame_bytes(),
            signal,
        },
        receiver,
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use kontainer_stream_protocol::StreamFrame;

    use tokio::sync::watch;

    use super::{OutboundFrameError, SessionSignal, outbound_queue};
    use crate::{StreamClose, StreamLimits};

    #[test]
    fn item_limit_rejects_overload_while_byte_capacity_remains() {
        let limits = StreamLimits::new(256, 1, 1024, 1, Duration::from_secs(1)).unwrap();
        let (outbound, _receiver) =
            outbound_queue(limits, watch::channel(SessionSignal::Running).0);

        outbound
            .try_send(StreamFrame::acknowledgement(1, vec![0; 80]))
            .unwrap();
        assert_eq!(
            outbound.try_send(StreamFrame::acknowledgement(2, vec![0; 8])),
            Err(OutboundFrameError::QueueOverloaded)
        );
    }

    #[test]
    fn byte_limit_rejects_overload_while_item_capacity_remains() {
        let limits = StreamLimits::new(256, 4, 128, 1, Duration::from_secs(1)).unwrap();
        let (outbound, _receiver) =
            outbound_queue(limits, watch::channel(SessionSignal::Running).0);

        outbound
            .try_send(StreamFrame::acknowledgement(1, vec![0; 80]))
            .unwrap();
        assert_eq!(
            outbound.try_send(StreamFrame::acknowledgement(2, vec![0; 80])),
            Err(OutboundFrameError::QueueOverloaded)
        );
    }

    #[test]
    fn an_application_close_finishes_once_and_never_overrides_an_earlier_close() {
        let limits = StreamLimits::default();
        let (signal, watcher) = watch::channel(SessionSignal::Running);
        let (outbound, _receiver) = outbound_queue(limits, signal.clone());
        outbound.close(StreamClose::new(4401, "access expired"));
        assert_eq!(
            *watcher.borrow(),
            SessionSignal::Finish(StreamClose::new(4401, "access expired"))
        );
        outbound.close(StreamClose::NORMAL);
        assert_eq!(
            *watcher.borrow(),
            SessionSignal::Finish(StreamClose::new(4401, "access expired"))
        );

        let (signal, watcher) = watch::channel(SessionSignal::Close(StreamClose::SLOW_CONSUMER));
        let (outbound, _receiver) = outbound_queue(limits, signal);
        outbound.close(StreamClose::NORMAL);
        assert_eq!(
            *watcher.borrow(),
            SessionSignal::Close(StreamClose::SLOW_CONSUMER)
        );
    }
}
