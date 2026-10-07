//! Interactive shells over the binary WebSocket transport, relayed to the node agent (spec 16,
//! ADR 0026).

pub mod domain;
pub mod relay;
pub mod session;

#[cfg(test)]
mod postgres_tests;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use kontainer_stream_protocol::StreamFrame;
use kontainer_stream_transport::{
    OutboundFrameError, OutboundSender, StreamApplication, StreamClose, StreamLimits, StreamServer,
};
use loco_rs::{Result, app::AppContext};

use crate::{
    exec::domain::{ExecTiming, MAX_EXEC_SESSIONS_PER_SESSION},
    stream_slots::SessionSlots,
};

/// The exec stream server and its per-session counts, in the shared store.
#[derive(Clone)]
pub struct ExecStreams {
    pub server: StreamServer,
    pub sessions: SessionSlots,
    /// Idle, maximum, and stall limits; tests shorten them.
    pub timing: ExecTiming,
}

/// Every connection is upgraded with its own session application.
struct NoSession;

#[async_trait]
impl StreamApplication for NoSession {
    async fn handle(
        &self,
        _frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        outbound.close(StreamClose::APPLICATION_FAILURE);
        Ok(())
    }
}

/// 64 KiB frames (ADR 0006) and a 256 KiB outbound queue per session.
fn exec_limits() -> StreamLimits {
    StreamLimits::new(64 * 1024, 64, 256 * 1024, 256, Duration::from_secs(2)).unwrap_or_default()
}

pub fn init_exec_streams(ctx: &AppContext) -> Result<()> {
    ctx.shared_store.insert(ExecStreams {
        server: StreamServer::new(exec_limits(), Arc::new(NoSession)),
        sessions: SessionSlots::new(MAX_EXEC_SESSIONS_PER_SESSION),
        timing: ExecTiming::default(),
    });
    Ok(())
}
