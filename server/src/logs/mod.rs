//! Service log streaming over the binary WebSocket transport (spec 15, ADR 0025).

pub mod backlog;
pub mod domain;
pub mod session;
pub mod stream;

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
    logs::domain::{LIVENESS_INTERVAL, MAX_LOG_CONNECTIONS_PER_SESSION},
    stream_slots::SessionSlots,
};

/// The log stream server and its per-session connection counts, in the shared store.
#[derive(Clone)]
pub struct LogStreams {
    pub server: StreamServer,
    pub connections: SessionSlots,
    /// How often a following stream checks the manager; tests shorten it.
    pub liveness: Duration,
}

/// Every connection is upgraded with its own session application, so the server-wide one only
/// answers a connection that somehow lacks one.
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

/// 64 KiB frames (ADR 0006), a deeper queue than the default for bursts of lines, and room for
/// every session's eight connections.
fn log_limits() -> StreamLimits {
    StreamLimits::new(64 * 1024, 256, 4 * 1024 * 1024, 256, Duration::from_secs(2))
        .unwrap_or_default()
}

pub fn init_log_streams(ctx: &AppContext) -> Result<()> {
    ctx.shared_store.insert(LogStreams {
        server: StreamServer::new(log_limits(), Arc::new(NoSession)),
        connections: SessionSlots::new(MAX_LOG_CONNECTIONS_PER_SESSION),
        liveness: LIVENESS_INTERVAL,
    });
    Ok(())
}
