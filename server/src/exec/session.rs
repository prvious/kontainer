//! One exec connection: the stream application the transport runs for it. It opens one shell,
//! relays the client's input and resizes to it, and stops the relay when dropped.

use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use kontainer_stream_protocol::{ExecError, ExecMessage, StreamFrame};
use kontainer_stream_transport::{
    OutboundFrameError, OutboundSender, StreamApplication, StreamClose,
};
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    AUDIT_TARGET,
    auth::domain::ValidatedSession,
    exec::{
        domain::{ExecTiming, valid_dimensions, validate_open},
        relay::{Activity, ExecRelay},
    },
    nodes::agents::AgentDirectory,
    rpc::AgentExecInput,
    stream_slots::SessionSlot,
};

const UNEXPECTED_FRAME: StreamClose = StreamClose::new(1008, "unexpected exec frame");
/// Input waiting for the agent; a full queue slows the client's reads rather than dropping keys.
const INPUT_QUEUE: usize = 64;

enum State {
    Waiting,
    Open {
        target: String,
        input: mpsc::Sender<AgentExecInput>,
        activity: Activity,
        cancellation: CancellationToken,
    },
}

pub struct ExecSession {
    database: DatabaseConnection,
    agents: Option<Arc<dyn AgentDirectory>>,
    session: ValidatedSession,
    timing: ExecTiming,
    shutdown: CancellationToken,
    state: Mutex<State>,
    _slot: SessionSlot,
}

impl ExecSession {
    #[must_use]
    pub fn new(
        database: DatabaseConnection,
        agents: Option<Arc<dyn AgentDirectory>>,
        session: ValidatedSession,
        timing: ExecTiming,
        shutdown: CancellationToken,
        slot: SessionSlot,
    ) -> Self {
        Self {
            database,
            agents,
            session,
            timing,
            shutdown,
            state: Mutex::new(State::Waiting),
            _slot: slot,
        }
    }

    fn open_input(&self) -> Option<(mpsc::Sender<AgentExecInput>, Activity)> {
        match &*self.state.lock().unwrap_or_else(PoisonError::into_inner) {
            State::Open {
                input, activity, ..
            } => Some((input.clone(), activity.clone())),
            State::Waiting => None,
        }
    }
}

#[async_trait]
impl StreamApplication for ExecSession {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        match ExecMessage::from_frame(&frame) {
            Ok(ExecMessage::Open(open)) => {
                let Some((node_id, agent_open)) = validate_open(&open) else {
                    outbound.try_send(
                        ExecMessage::Error(ExecError {
                            code: "invalid_request".to_owned(),
                            message: "The exec request was refused.".to_owned(),
                        })
                        .into_frame(1),
                    )?;
                    outbound.close(StreamClose::NORMAL);
                    return Ok(());
                };
                let (sender, receiver) = mpsc::channel(INPUT_QUEUE);
                let activity = Activity::now();
                let cancellation = self.shutdown.child_token();
                {
                    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                    if !matches!(*state, State::Waiting) {
                        // One connection runs one shell.
                        outbound.close(UNEXPECTED_FRAME);
                        return Ok(());
                    }
                    *state = State::Open {
                        target: format!("{node_id}/{}", agent_open.container_id),
                        input: sender.clone(),
                        activity: activity.clone(),
                        cancellation: cancellation.clone(),
                    };
                }
                tracing::info!(
                    target: AUDIT_TARGET,
                    event = "exec.open",
                    principal = %self.session.principal_id,
                    session = %self.session.family_id,
                    node = %node_id,
                    container = %agent_open.container_id,
                );
                let relay = ExecRelay {
                    database: self.database.clone(),
                    agents: self.agents.clone(),
                    outbound: outbound.clone(),
                    node_id,
                    open: agent_open,
                    input: receiver,
                    closer: sender,
                    activity,
                    principal_id: self.session.principal_id,
                    family_id: self.session.family_id,
                    access_expires_at: self.session.access_expires_at,
                    timing: self.timing,
                    cancellation,
                };
                tokio::spawn(relay.run());
                Ok(())
            }
            Ok(ExecMessage::Input(bytes)) => {
                if let Some((input, activity)) = self.open_input() {
                    activity.touch();
                    let _ = input.send(AgentExecInput::Input(bytes)).await;
                }
                Ok(())
            }
            Ok(ExecMessage::Resize(resize)) if valid_dimensions(resize.cols, resize.rows) => {
                if let Some((input, _)) = self.open_input() {
                    let _ = input
                        .send(AgentExecInput::Resize {
                            cols: resize.cols,
                            rows: resize.rows,
                        })
                        .await;
                }
                Ok(())
            }
            Ok(ExecMessage::Close) => {
                if let Some((input, _)) = self.open_input() {
                    let _ = input.send(AgentExecInput::Close).await;
                }
                Ok(())
            }
            _ => {
                outbound.close(UNEXPECTED_FRAME);
                Ok(())
            }
        }
    }
}

impl Drop for ExecSession {
    fn drop(&mut self) {
        let state = self.state.get_mut().unwrap_or_else(PoisonError::into_inner);
        if let State::Open {
            target,
            input,
            cancellation,
            ..
        } = state
        {
            let _ = input.try_send(AgentExecInput::Close);
            cancellation.cancel();
            tracing::info!(
                target: AUDIT_TARGET,
                event = "exec.close",
                principal = %self.session.principal_id,
                session = %self.session.family_id,
                target = %target,
            );
        }
    }
}
