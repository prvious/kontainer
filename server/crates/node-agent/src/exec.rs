//! Exec sessions on this node: one TTY shell per `Exec` stream (ADR 0026).
//!
//! The first client frame opens the session. A container that isn't running is refused; without
//! a requested shell the agent tries `/bin/bash -l` and falls back to `/bin/sh` when bash exits
//! within 500 ms with 126 or 127. Output, input, and resizes then flow until the shell exits or
//! the controller closes, and the exit code comes from the engine, never made up. A shell still
//! running after the close is reported as still running and left alone (ADR 0033).

use std::{pin::Pin, sync::Arc, time::Duration};

use futures_util::{Stream, StreamExt};
use kontainer_agent_proto::v1::{
    ExecClientFrame, ExecErrorEvent, ExecExitEvent, ExecOpenRequest, ExecServerFrame,
    ExecStartedEvent, exec_client_frame, exec_server_frame,
};
use kontainer_engine::{
    ContainerEngine, ExecAttachment, ExecStatus, is_missing_shell_status, is_valid_container_id,
};
use tokio::{sync::mpsc, time};
use tokio_util::sync::CancellationToken;
use tonic::Status;

pub type ExecStream = Pin<Box<dyn Stream<Item = Result<ExecServerFrame, Status>> + Send>>;

/// A shell that ends this soon with 126 or 127 was never there.
pub const SHELL_PROBE: Duration = Duration::from_millis(500);
/// After the controller closes, the shell gets this long to exit on its own.
pub const CLOSE_GRACE: Duration = Duration::from_secs(2);
const DEFAULT_SHELL: [&str; 2] = ["/bin/bash", "-l"];
const FALLBACK_SHELL: &str = "/bin/sh";
const MAX_SHELL_LEN: usize = 256;
const MAX_DIMENSION: u32 = 1_000;
const TERM: &str = "TERM=xterm-256color";

fn frame(frame: exec_server_frame::Frame) -> Result<ExecServerFrame, Status> {
    Ok(ExecServerFrame { frame: Some(frame) })
}

fn error(code: &str, message: &str) -> Result<ExecServerFrame, Status> {
    frame(exec_server_frame::Frame::Error(ExecErrorEvent {
        code: code.to_owned(),
        message: message.to_owned(),
    }))
}

/// An open that can be tried: a real container id, a sane shell, and a real terminal size.
pub fn validate_open(open: &ExecOpenRequest) -> Result<(u16, u16), Status> {
    let dimension = |value: u32| {
        u16::try_from(value)
            .ok()
            .filter(|value| *value > 0 && u32::from(*value) <= MAX_DIMENSION)
    };
    let shell_ok = open.shell.as_deref().is_none_or(|shell| {
        !shell.is_empty() && shell.len() <= MAX_SHELL_LEN && !shell.chars().any(char::is_control)
    });
    match (dimension(open.cols), dimension(open.rows)) {
        (Some(cols), Some(rows)) if is_valid_container_id(&open.container_id) && shell_ok => {
            Ok((cols, rows))
        }
        _ => Err(Status::invalid_argument("invalid_request")),
    }
}

/// What the controller sends: tonic's `Streaming` in production, any stream in tests.
pub trait ExecInbound:
    Stream<Item = Result<ExecClientFrame, Status>> + Unpin + Send + 'static
{
}

impl<T> ExecInbound for T where
    T: Stream<Item = Result<ExecClientFrame, Status>> + Unpin + Send + 'static
{
}

/// Take the open frame and start the session in its own task; the stream carries its frames.
pub async fn open(
    engine: Arc<dyn ContainerEngine>,
    mut inbound: impl ExecInbound,
    cancellation: CancellationToken,
) -> Result<ExecStream, Status> {
    let first = inbound
        .next()
        .await
        .transpose()?
        .and_then(|first| first.frame)
        .ok_or_else(|| Status::invalid_argument("invalid_request"))?;
    let exec_client_frame::Frame::Open(open) = first else {
        return Err(Status::invalid_argument("invalid_request"));
    };
    let size = validate_open(&open)?;
    let (sender, receiver) = mpsc::channel(64);
    tokio::spawn(async move {
        let session = Session {
            engine,
            sender,
            cancellation,
        };
        session.run(open, size, inbound).await;
    });
    Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(
        receiver,
    )))
}

struct Session {
    engine: Arc<dyn ContainerEngine>,
    sender: mpsc::Sender<Result<ExecServerFrame, Status>>,
    cancellation: CancellationToken,
}

struct Started {
    exec_id: String,
    shell: String,
    attachment: ExecAttachment,
    early: Vec<u8>,
    /// The shell already exited inside the probe window.
    ended: bool,
}

enum StartFailure {
    MissingShell,
    Engine,
}

impl Session {
    async fn send(&self, frame: Result<ExecServerFrame, Status>) -> bool {
        self.sender.send(frame).await.is_ok()
    }

    async fn run(
        self,
        open: ExecOpenRequest,
        (cols, rows): (u16, u16),
        mut inbound: impl ExecInbound,
    ) {
        match self.engine.running_container_ids(&self.cancellation).await {
            Ok(ids) if ids.contains(&open.container_id) => {}
            Ok(_) => {
                self.send(error("target_not_running", "The container isn't running."))
                    .await;
                return;
            }
            Err(_) => {
                self.send(error("engine_error", "Docker didn't answer on this node."))
                    .await;
                return;
            }
        }
        let candidates: Vec<(Vec<String>, String)> = match &open.shell {
            Some(shell) => vec![(vec![shell.clone()], shell.clone())],
            None => vec![
                (
                    DEFAULT_SHELL
                        .iter()
                        .map(|part| (*part).to_owned())
                        .collect(),
                    DEFAULT_SHELL[0].to_owned(),
                ),
                (vec![FALLBACK_SHELL.to_owned()], FALLBACK_SHELL.to_owned()),
            ],
        };
        let mut started = None;
        for (command, shell) in candidates {
            match self
                .start(&open.container_id, &command, shell, cols, rows)
                .await
            {
                Ok(shell) => {
                    started = Some(shell);
                    break;
                }
                Err(StartFailure::MissingShell) => {}
                Err(StartFailure::Engine) => {
                    self.send(error("engine_error", "The shell could not be started."))
                        .await;
                    return;
                }
            }
        }
        let Some(started) = started else {
            let message = match &open.shell {
                Some(shell) => format!("No shell at {shell} in this container."),
                None => "Neither /bin/bash nor /bin/sh exists in this container.".to_owned(),
            };
            self.send(error("shell_not_found", &message)).await;
            return;
        };
        self.relay(started, &mut inbound).await;
    }

    async fn start(
        &self,
        container_id: &str,
        command: &[String],
        shell: String,
        cols: u16,
        rows: u16,
    ) -> Result<Started, StartFailure> {
        let exec_id = self
            .engine
            .exec_create(
                container_id,
                command,
                &[TERM.to_owned()],
                &self.cancellation,
            )
            .await
            .map_err(|_| StartFailure::Engine)?;
        let mut attachment = self
            .engine
            .exec_start(&exec_id, &self.cancellation)
            .await
            .map_err(|_| StartFailure::Engine)?;
        let _ = self
            .engine
            .exec_resize(&exec_id, cols, rows, &self.cancellation)
            .await;
        let mut early = Vec::new();
        let probe = time::sleep(SHELL_PROBE);
        tokio::pin!(probe);
        let ended = loop {
            tokio::select! {
                () = &mut probe => break false,
                item = attachment.output.next() => match item {
                    Some(Ok(bytes)) => early.extend_from_slice(&bytes),
                    Some(Err(_)) | None => break true,
                },
            }
        };
        if ended
            && is_missing_shell_status(
                self.final_status(&exec_id)
                    .await
                    .and_then(|status| status.exit_code),
            )
        {
            return Err(StartFailure::MissingShell);
        }
        Ok(Started {
            exec_id,
            shell,
            attachment,
            early,
            ended,
        })
    }

    /// The process's status once it has stopped, waiting up to [`CLOSE_GRACE`] for it: still
    /// `running` when it didn't stop in time, `None` when the engine didn't answer.
    async fn final_status(&self, exec_id: &str) -> Option<ExecStatus> {
        let deadline = time::Instant::now() + CLOSE_GRACE;
        loop {
            match self.engine.exec_inspect(exec_id, &self.cancellation).await {
                Ok(status) if status.running && time::Instant::now() < deadline => {
                    time::sleep(Duration::from_millis(50)).await;
                }
                Ok(status) => return Some(status),
                Err(_) => return None,
            }
        }
    }

    async fn relay(self, started: Started, inbound: &mut impl ExecInbound) {
        let Started {
            exec_id,
            shell,
            mut attachment,
            early,
            ended,
        } = started;
        if !self
            .send(frame(exec_server_frame::Frame::Started(ExecStartedEvent {
                exec_id: exec_id.clone(),
                shell,
            })))
            .await
        {
            return;
        }
        if !early.is_empty()
            && !self
                .send(frame(exec_server_frame::Frame::Output(early)))
                .await
        {
            return;
        }
        let mut closing: Option<time::Instant> = None;
        let mut output_done = ended;
        while !output_done {
            let grace = closing.map_or_else(
                || time::Instant::now() + Duration::from_secs(86_400),
                |since| since + CLOSE_GRACE,
            );
            tokio::select! {
                biased;
                () = self.cancellation.cancelled() => {
                    attachment.input.close().await;
                    break;
                }
                () = time::sleep_until(grace), if closing.is_some() => break,
                item = attachment.output.next() => match item {
                    Some(Ok(bytes)) => {
                        if !self.send(frame(exec_server_frame::Frame::Output(bytes))).await {
                            attachment.input.close().await;
                            return;
                        }
                    }
                    Some(Err(_)) | None => output_done = true,
                },
                message = inbound.next(), if closing.is_none() => match message.and_then(Result::ok).and_then(|message| message.frame) {
                    Some(exec_client_frame::Frame::Input(bytes)) => {
                        if attachment.input.write(&bytes).await.is_err() {
                            output_done = true;
                        }
                    }
                    Some(exec_client_frame::Frame::Resize(resize)) => {
                        if let (Ok(cols), Ok(rows)) = (u16::try_from(resize.cols), u16::try_from(resize.rows)) {
                            let _ = self.engine.exec_resize(&exec_id, cols, rows, &self.cancellation).await;
                        }
                    }
                    Some(exec_client_frame::Frame::Open(_)) => {}
                    // A close, or the controller went away: end of input, then a short wait.
                    Some(exec_client_frame::Frame::Close(_)) | None => {
                        attachment.input.close().await;
                        closing = Some(time::Instant::now());
                    }
                },
            }
        }
        // Closing a TTY's input ends its output but not the process, so a shell can outlive its
        // session; it is reported as still running, not as exited.
        let status = self.final_status(&exec_id).await;
        self.send(frame(exec_server_frame::Frame::Exit(ExecExitEvent {
            code: status.and_then(|status| status.exit_code),
            still_running: status.is_some_and(|status| status.running),
        })))
        .await;
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        io,
        pin::Pin,
        sync::{Arc, Mutex},
        task::{Context, Poll},
        time::Duration,
    };

    use async_trait::async_trait;
    use futures_util::StreamExt;
    use kontainer_agent_proto::v1::{
        ExecClientFrame, ExecCloseRequest, ExecOpenRequest, ExecResizeRequest, ExecServerFrame,
        exec_client_frame, exec_server_frame,
    };
    use kontainer_engine::{
        ContainerEngine, EngineError, EngineErrorKind, EngineOperation, ExecAttachment, ExecInput,
        ExecStatus,
    };
    use tokio::{io::AsyncWrite, sync::mpsc};
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tokio_util::sync::CancellationToken;
    use tonic::Status;

    use super::open;

    const CONTAINER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl AsyncWrite for Captured {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Poll::Ready(Ok(bytes.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    /// Shells named in `missing` exit at once with 127; any other shell reads output from the
    /// test and exits with `exit_code` once that output ends.
    #[derive(Default)]
    struct FakeEngine {
        running: bool,
        missing: Vec<&'static str>,
        exit_code: Option<i64>,
        /// The shell never stops, as a TTY shell whose input closed doesn't.
        still_running: bool,
        commands: Mutex<HashMap<String, Vec<String>>>,
        resized: Mutex<Vec<(u16, u16)>>,
        written: Arc<Mutex<Vec<u8>>>,
        output: Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>,
    }

    impl FakeEngine {
        fn missing_shell(&self, exec_id: &str) -> bool {
            let commands = self.commands.lock().unwrap();
            self.missing
                .iter()
                .any(|shell| commands[exec_id][0] == *shell)
        }
    }

    #[async_trait]
    impl ContainerEngine for FakeEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn running_container_ids(
            &self,
            _: &CancellationToken,
        ) -> Result<Vec<String>, EngineError> {
            Ok(if self.running {
                vec![CONTAINER.to_owned()]
            } else {
                Vec::new()
            })
        }

        async fn exec_create(
            &self,
            _: &str,
            command: &[String],
            env: &[String],
            _: &CancellationToken,
        ) -> Result<String, EngineError> {
            assert_eq!(env, ["TERM=xterm-256color".to_owned()]);
            let mut commands = self.commands.lock().unwrap();
            let id = format!("{:064x}", commands.len() + 1);
            commands.insert(id.clone(), command.to_vec());
            Ok(id)
        }

        async fn exec_start(
            &self,
            exec_id: &str,
            _: &CancellationToken,
        ) -> Result<ExecAttachment, EngineError> {
            let output: kontainer_engine::ExecOutput = if self.missing_shell(exec_id) {
                Box::pin(futures_util::stream::empty())
            } else {
                let receiver = self.output.lock().unwrap().take().expect("one live shell");
                Box::pin(UnboundedReceiverStream::new(receiver).map(Ok))
            };
            Ok(ExecAttachment {
                output,
                input: ExecInput::new(Box::pin(Captured(self.written.clone()))),
            })
        }

        async fn exec_resize(
            &self,
            _: &str,
            cols: u16,
            rows: u16,
            _: &CancellationToken,
        ) -> Result<(), EngineError> {
            self.resized.lock().unwrap().push((cols, rows));
            Ok(())
        }

        async fn exec_inspect(
            &self,
            exec_id: &str,
            _: &CancellationToken,
        ) -> Result<ExecStatus, EngineError> {
            if self.missing_shell(exec_id) {
                return Ok(ExecStatus {
                    running: false,
                    exit_code: Some(127),
                });
            }
            if self.still_running {
                return Ok(ExecStatus {
                    running: true,
                    exit_code: None,
                });
            }
            match self.exit_code {
                Some(code) => Ok(ExecStatus {
                    running: false,
                    exit_code: Some(code),
                }),
                None => Err(EngineError::new(
                    EngineErrorKind::DaemonRejected,
                    EngineOperation::Exec,
                )),
            }
        }
    }

    fn client(frame: exec_client_frame::Frame) -> Result<ExecClientFrame, Status> {
        Ok(ExecClientFrame { frame: Some(frame) })
    }

    fn open_frame(shell: Option<&str>) -> Result<ExecClientFrame, Status> {
        client(exec_client_frame::Frame::Open(ExecOpenRequest {
            container_id: CONTAINER.to_owned(),
            shell: shell.map(str::to_owned),
            cols: 120,
            rows: 40,
        }))
    }

    async fn session(
        engine: Arc<FakeEngine>,
        shell: Option<&str>,
    ) -> (
        mpsc::UnboundedSender<Result<ExecClientFrame, Status>>,
        Pin<Box<dyn futures_util::Stream<Item = Result<ExecServerFrame, Status>> + Send>>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();
        sender.send(open_frame(shell)).unwrap();
        let stream = open(
            engine,
            UnboundedReceiverStream::new(receiver),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        (sender, stream)
    }

    async fn next(
        stream: &mut Pin<
            Box<dyn futures_util::Stream<Item = Result<ExecServerFrame, Status>> + Send>,
        >,
    ) -> exec_server_frame::Frame {
        tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("a frame within 5 s")
            .expect("the stream is open")
            .unwrap()
            .frame
            .unwrap()
    }

    #[tokio::test]
    async fn a_stopped_container_and_a_missing_custom_shell_are_refused() {
        let stopped = Arc::new(FakeEngine::default());
        let (_input, mut stream) = session(stopped, None).await;
        let exec_server_frame::Frame::Error(error) = next(&mut stream).await else {
            panic!("an error");
        };
        assert_eq!(error.code, "target_not_running");

        let engine = Arc::new(FakeEngine {
            running: true,
            missing: vec!["/bin/zsh"],
            ..FakeEngine::default()
        });
        let (_input, mut stream) = session(engine.clone(), Some("/bin/zsh")).await;
        let exec_server_frame::Frame::Error(error) = next(&mut stream).await else {
            panic!("an error");
        };
        assert_eq!(error.code, "shell_not_found");
        assert_eq!(
            engine.commands.lock().unwrap().len(),
            1,
            "a given shell has no fallback"
        );
    }

    #[tokio::test]
    async fn without_bash_it_falls_back_to_sh_and_relays_until_the_exit_code() {
        let (output, receiver) = mpsc::unbounded_channel();
        let engine = Arc::new(FakeEngine {
            running: true,
            missing: vec!["/bin/bash"],
            exit_code: Some(3),
            output: Mutex::new(Some(receiver)),
            ..FakeEngine::default()
        });
        output.send(b"$ ".to_vec()).unwrap();
        let (input, mut stream) = session(engine.clone(), None).await;
        let exec_server_frame::Frame::Started(started) = next(&mut stream).await else {
            panic!("started");
        };
        assert_eq!(started.shell, "/bin/sh");
        let commands = engine.commands.lock().unwrap().clone();
        let mut shells: Vec<_> = commands.values().map(|command| command.join(" ")).collect();
        shells.sort();
        assert_eq!(shells, vec!["/bin/bash -l", "/bin/sh"]);
        assert_eq!(
            next(&mut stream).await,
            exec_server_frame::Frame::Output(b"$ ".to_vec()),
            "what the shell wrote while it was probed"
        );

        input
            .send(client(exec_client_frame::Frame::Input(
                b"echo ok\r".to_vec(),
            )))
            .unwrap();
        input
            .send(client(exec_client_frame::Frame::Resize(
                ExecResizeRequest { cols: 90, rows: 20 },
            )))
            .unwrap();
        output.send(b"ok\r\n".to_vec()).unwrap();
        assert_eq!(
            next(&mut stream).await,
            exec_server_frame::Frame::Output(b"ok\r\n".to_vec())
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(engine.written.lock().unwrap().as_slice(), b"echo ok\r");
        assert_eq!(
            engine.resized.lock().unwrap().last(),
            Some(&(90, 20)),
            "the open size first, then the resize"
        );

        input
            .send(client(exec_client_frame::Frame::Close(ExecCloseRequest {})))
            .unwrap();
        drop(output);
        let exec_server_frame::Frame::Exit(exit) = next(&mut stream).await else {
            panic!("an exit");
        };
        assert_eq!((exit.code, exit.still_running), (Some(3), false));
    }

    #[tokio::test]
    async fn an_unknown_exit_status_is_reported_as_unknown_not_zero() {
        let (output, receiver) = mpsc::unbounded_channel();
        let engine = Arc::new(FakeEngine {
            running: true,
            exit_code: None,
            output: Mutex::new(Some(receiver)),
            ..FakeEngine::default()
        });
        let (_input, mut stream) = session(engine, Some("/bin/sh")).await;
        assert!(matches!(
            next(&mut stream).await,
            exec_server_frame::Frame::Started(_)
        ));
        drop(output);
        let exec_server_frame::Frame::Exit(exit) = next(&mut stream).await else {
            panic!("an exit");
        };
        assert_eq!((exit.code, exit.still_running), (None, false));
    }

    #[tokio::test]
    async fn a_shell_still_running_after_the_close_grace_is_reported_as_still_running() {
        let (output, receiver) = mpsc::unbounded_channel();
        let engine = Arc::new(FakeEngine {
            running: true,
            still_running: true,
            output: Mutex::new(Some(receiver)),
            ..FakeEngine::default()
        });
        let (input, mut stream) = session(engine, Some("/bin/sh")).await;
        assert!(matches!(
            next(&mut stream).await,
            exec_server_frame::Frame::Started(_)
        ));
        // Docker ends the output once the input closes; the shell carries on.
        input
            .send(client(exec_client_frame::Frame::Close(ExecCloseRequest {})))
            .unwrap();
        drop(output);
        let exec_server_frame::Frame::Exit(exit) = next(&mut stream).await else {
            panic!("an exit");
        };
        assert_eq!((exit.code, exit.still_running), (None, true));
    }

    #[test]
    fn opens_need_a_container_id_a_sane_shell_and_a_real_size() {
        let open = |container: &str, shell: Option<&str>, cols, rows| ExecOpenRequest {
            container_id: container.to_owned(),
            shell: shell.map(str::to_owned),
            cols,
            rows,
        };
        assert_eq!(
            super::validate_open(&open(CONTAINER, None, 80, 24)).unwrap(),
            (80, 24)
        );
        for bad in [
            open("web", None, 80, 24),
            open(CONTAINER, Some(""), 80, 24),
            open(CONTAINER, Some("/bin/sh\n"), 80, 24),
            open(CONTAINER, None, 0, 24),
            open(CONTAINER, None, 80, 5_000),
        ] {
            assert!(super::validate_open(&bad).is_err(), "{bad:?}");
        }
    }
}
