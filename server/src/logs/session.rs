//! One log connection: the stream application the transport runs for it, carrying the session
//! it was authenticated as. It follows at most one service and stops the follow when dropped.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use kontainer_stream_protocol::{LogMessage, StreamFrame};
use kontainer_stream_transport::{
    OutboundFrameError, OutboundSender, StreamApplication, StreamClose,
};
use tokio_util::sync::CancellationToken;

use crate::{
    auth::domain::ValidatedSession, logs::stream::LogStream, stream_slots::SessionSlot,
    swarm::engine::ManagerEngine,
};

const UNEXPECTED_FRAME: StreamClose = StreamClose::new(1008, "unexpected log frame");

pub struct LogsSession {
    manager: Arc<dyn ManagerEngine>,
    session: ValidatedSession,
    liveness: Duration,
    shutdown: CancellationToken,
    follow: Mutex<Option<(String, CancellationToken)>>,
    _slot: SessionSlot,
}

impl LogsSession {
    #[must_use]
    pub fn new(
        manager: Arc<dyn ManagerEngine>,
        session: ValidatedSession,
        liveness: Duration,
        shutdown: CancellationToken,
        slot: SessionSlot,
    ) -> Self {
        Self {
            manager,
            session,
            liveness,
            shutdown,
            follow: Mutex::new(None),
            _slot: slot,
        }
    }
}

#[async_trait]
impl StreamApplication for LogsSession {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        match LogMessage::from_frame(&frame) {
            Ok(LogMessage::Open(open)) => {
                let cancellation = self.shutdown.child_token();
                {
                    let mut follow = self
                        .follow
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if follow.is_some() {
                        // One connection follows one service.
                        outbound.close(UNEXPECTED_FRAME);
                        return Ok(());
                    }
                    *follow = Some((open.service_id.clone(), cancellation.clone()));
                }
                tracing::info!(
                    target: crate::AUDIT_TARGET,
                    event = "logs.open",
                    principal = %self.session.principal_id,
                    session = %self.session.family_id,
                    service = %open.service_id,
                );
                let stream = LogStream {
                    manager: self.manager.clone(),
                    outbound: outbound.clone(),
                    access_expires_at: self.session.access_expires_at,
                    liveness: self.liveness,
                    cancellation,
                };
                tokio::spawn(stream.run(open));
                Ok(())
            }
            _ => {
                outbound.close(UNEXPECTED_FRAME);
                Ok(())
            }
        }
    }
}

/// The client closed the socket, or the transport ended the session: stop following, and record
/// it without content.
impl Drop for LogsSession {
    fn drop(&mut self) {
        let stopped = self
            .follow
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some((service_id, cancellation)) = stopped {
            cancellation.cancel();
            tracing::info!(
                target: crate::AUDIT_TARGET,
                event = "logs.close",
                principal = %self.session.principal_id,
                session = %self.session.family_id,
                service = %service_id,
            );
        }
    }
}
