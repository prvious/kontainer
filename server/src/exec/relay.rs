//! One exec session's relay: resolve the node, open the agent's `Exec` stream, and carry its
//! events to the connection until the shell exits, the session times out, the credential
//! expires, the client stalls, or either side goes away.

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use kontainer_stream_protocol::{ExecError, ExecExit, ExecMessage, ExecStarted};
use kontainer_stream_transport::{OutboundFrameError, OutboundSender, StreamClose};
use sea_orm::{DatabaseConnection, EntityTrait};
use tokio::{sync::mpsc, time};
use tokio_util::sync::CancellationToken;

use crate::{
    exec::domain::{ExecTiming, ExitReason, OUTPUT_CHUNK, client_error_code},
    nodes::{
        agents::{AgentDirectory, NodeEndpoint},
        entities::node,
    },
    rpc::{AgentClient, AgentExecEvent, AgentExecInput, AgentExecOpen},
};

/// Closed when the access credential the connection opened with expires.
pub const ACCESS_EXPIRED: StreamClose = StreamClose::new(4401, "access expired");
const RETRY_EVERY: Duration = Duration::from_millis(25);

/// When the last input or output happened, shared with the session that sees input.
#[derive(Clone)]
pub struct Activity(Arc<Mutex<time::Instant>>);

impl Activity {
    #[must_use]
    pub fn now() -> Self {
        Self(Arc::new(Mutex::new(time::Instant::now())))
    }

    pub fn touch(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = time::Instant::now();
    }

    fn last(&self) -> time::Instant {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct ExecRelay {
    pub database: DatabaseConnection,
    pub agents: Option<Arc<dyn AgentDirectory>>,
    pub outbound: OutboundSender,
    pub node_id: String,
    pub open: AgentExecOpen,
    /// What the session relays from the client; the relay keeps a sender to say close itself.
    pub input: mpsc::Receiver<AgentExecInput>,
    pub closer: mpsc::Sender<AgentExecInput>,
    pub activity: Activity,
    pub access_expires_at: DateTime<Utc>,
    pub timing: ExecTiming,
    pub cancellation: CancellationToken,
}

enum Stop {
    Stalled,
    Gone,
}

struct Frames {
    outbound: OutboundSender,
    sequence: u64,
    stall: Duration,
}

impl Frames {
    /// Send, waiting up to the stall limit for room; a client that doesn't make room in time is
    /// dropped.
    async fn send(&mut self, message: ExecMessage) -> Result<(), Stop> {
        let frame = message.into_frame(self.sequence + 1);
        let deadline = time::Instant::now() + self.stall;
        loop {
            match self.outbound.try_send(frame.clone()) {
                Ok(()) => {
                    self.sequence += 1;
                    return Ok(());
                }
                Err(OutboundFrameError::QueueOverloaded) if time::Instant::now() < deadline => {
                    time::sleep(RETRY_EVERY).await;
                }
                Err(OutboundFrameError::QueueOverloaded) => return Err(Stop::Stalled),
                Err(_) => return Err(Stop::Gone),
            }
        }
    }

    /// Send a last frame and close normally.
    async fn finish(&mut self, message: ExecMessage) {
        match self.send(message).await {
            Ok(()) => self.outbound.close(StreamClose::NORMAL),
            Err(Stop::Stalled) => self.outbound.close(StreamClose::SLOW_CONSUMER),
            Err(Stop::Gone) => {}
        }
    }

    /// Tell the agent to close and report `reason` rather than wait for it.
    async fn end(&mut self, closer: &mpsc::Sender<AgentExecInput>, reason: ExitReason) {
        let _ = closer.try_send(AgentExecInput::Close);
        self.finish(ExecMessage::Exit(ExecExit {
            code: None,
            reason: reason.as_str().to_owned(),
        }))
        .await;
    }

    async fn fail(&mut self, code: &str, message: &str) {
        self.finish(ExecMessage::Error(ExecError {
            code: code.to_owned(),
            message: message.to_owned(),
        }))
        .await;
    }
}

impl ExecRelay {
    async fn client(&self) -> Result<Arc<dyn AgentClient>, &'static str> {
        let agents = self.agents.as_ref().ok_or("node_unavailable")?;
        let member = node::Entity::find_by_id(self.node_id.clone())
            .one(&self.database)
            .await
            .map_err(|_| "node_unavailable")?
            .ok_or("node_unavailable")?;
        agents
            .client(&NodeEndpoint {
                node_id: member.id,
                advertise_address: member.advertise_address,
            })
            .map_err(|_| "node_unavailable")
    }

    pub async fn run(self) {
        let mut frames = Frames {
            outbound: self.outbound.clone(),
            sequence: 0,
            stall: self.timing.stall,
        };
        let client = match self.client().await {
            Ok(client) => client,
            Err(code) => {
                return frames
                    .fail(code, "The node's agent can't be reached.")
                    .await;
            }
        };
        let Self {
            outbound,
            open,
            input,
            closer,
            activity,
            access_expires_at,
            timing,
            cancellation,
            ..
        } = self;
        let mut events = match client.exec(open, input, &cancellation).await {
            Ok(events) => events,
            Err(_) => {
                return frames
                    .fail("node_unavailable", "The node's agent can't be reached.")
                    .await;
            }
        };
        let started = time::Instant::now();
        let expiry = time::sleep(
            (access_expires_at - Utc::now())
                .to_std()
                .unwrap_or_default(),
        );
        tokio::pin!(expiry);
        loop {
            let idle_at = activity.last() + timing.idle;
            tokio::select! {
                biased;
                () = cancellation.cancelled() => return,
                () = outbound.closed() => return,
                () = &mut expiry => {
                    let _ = closer.try_send(AgentExecInput::Close);
                    let _ = frames.send(ExecMessage::Error(ExecError {
                        code: "session_expired".to_owned(),
                        message: "The session's access expired; sign in again to reconnect.".to_owned(),
                    })).await;
                    outbound.close(ACCESS_EXPIRED);
                    return;
                }
                () = time::sleep_until(started + timing.max) => {
                    return frames.end(&closer, ExitReason::MaxDuration).await;
                }
                () = time::sleep_until(idle_at) => {
                    // Input may have arrived since the deadline was computed.
                    if time::Instant::now() >= activity.last() + timing.idle {
                        return frames.end(&closer, ExitReason::IdleTimeout).await;
                    }
                }
                event = events.next() => {
                    let sent = match event {
                        Some(AgentExecEvent::Started { exec_id, shell }) => {
                            frames.send(ExecMessage::Started(ExecStarted { exec_id, shell })).await
                        }
                        Some(AgentExecEvent::Output(bytes)) => {
                            activity.touch();
                            let mut sent = Ok(());
                            for chunk in bytes.chunks(OUTPUT_CHUNK) {
                                sent = frames.send(ExecMessage::Output(chunk.to_vec())).await;
                                if sent.is_err() {
                                    break;
                                }
                            }
                            sent
                        }
                        Some(AgentExecEvent::Exit { code }) => {
                            return frames
                                .finish(ExecMessage::Exit(ExecExit {
                                    code,
                                    reason: ExitReason::Exited.as_str().to_owned(),
                                }))
                                .await;
                        }
                        Some(AgentExecEvent::Error { code, message }) => {
                            return frames.fail(client_error_code(&code), &message).await;
                        }
                        None => return frames.end(&closer, ExitReason::ConnectionLost).await,
                    };
                    match sent {
                        Ok(()) => {}
                        Err(Stop::Stalled) => {
                            let _ = closer.try_send(AgentExecInput::Close);
                            outbound.close(StreamClose::SLOW_CONSUMER);
                            return;
                        }
                        Err(Stop::Gone) => return,
                    }
                }
            }
        }
    }
}
