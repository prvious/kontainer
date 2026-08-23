use std::sync::Arc;

use async_trait::async_trait;
use kontainer_stream_protocol::StreamFrame;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

use crate::StreamLimits;

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
}

impl OutboundSender {
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
) -> (OutboundSender, mpsc::Receiver<QueuedFrame>) {
    let (sender, receiver) = mpsc::channel(limits.maximum_queued_frames());
    (
        OutboundSender {
            sender,
            byte_budget: Arc::new(Semaphore::new(limits.maximum_queued_bytes())),
            maximum_frame_bytes: limits.maximum_frame_bytes(),
        },
        receiver,
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use kontainer_stream_protocol::StreamFrame;

    use super::{OutboundFrameError, outbound_queue};
    use crate::StreamLimits;

    #[test]
    fn item_and_byte_limits_reject_overload_without_waiting() {
        let limits = StreamLimits::new(256, 1, 128, 1, Duration::from_secs(1)).unwrap();
        let (outbound, _receiver) = outbound_queue(limits);

        outbound
            .try_send(StreamFrame::acknowledgement(1, vec![0; 80]))
            .unwrap();
        assert_eq!(
            outbound.try_send(StreamFrame::acknowledgement(2, vec![0; 8])),
            Err(OutboundFrameError::QueueOverloaded)
        );
    }
}
