//! Interactive exec sessions in Kontainer's own terms: a TTY process in a running container with
//! its input, its output bytes, and its exit status.

use std::{future::poll_fn, pin::Pin};

use futures_util::Stream;
use tokio::io::AsyncWrite;

use crate::{EngineError, EngineErrorKind, EngineOperation};

/// Raw output bytes, in the order the TTY wrote them.
pub type ExecOutput = Pin<Box<dyn Stream<Item = Result<Vec<u8>, EngineError>> + Send>>;

/// The process's standard input.
pub struct ExecInput(Pin<Box<dyn AsyncWrite + Send>>);

impl ExecInput {
    #[must_use]
    pub fn new(writer: Pin<Box<dyn AsyncWrite + Send>>) -> Self {
        Self(writer)
    }

    /// Write every byte and flush.
    pub async fn write(&mut self, mut bytes: &[u8]) -> Result<(), EngineError> {
        let failed = || EngineError::new(EngineErrorKind::DaemonRejected, EngineOperation::Exec);
        while !bytes.is_empty() {
            let written = poll_fn(|context| self.0.as_mut().poll_write(context, bytes))
                .await
                .map_err(|_| failed())?;
            if written == 0 {
                return Err(failed());
            }
            bytes = &bytes[written..];
        }
        poll_fn(|context| self.0.as_mut().poll_flush(context))
            .await
            .map_err(|_| failed())
    }

    /// Close standard input; the shell sees end of file.
    pub async fn close(&mut self) {
        let _ = poll_fn(|context| self.0.as_mut().poll_shutdown(context)).await;
    }
}

/// A started exec's streams.
pub struct ExecAttachment {
    pub output: ExecOutput,
    pub input: ExecInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExecStatus {
    pub running: bool,
    /// `None` while running, or when the daemon doesn't know it.
    pub exit_code: Option<i64>,
}

/// Exit statuses a shell that could not be found or run ends with.
#[must_use]
pub fn is_missing_shell_status(code: Option<i64>) -> bool {
    matches!(code, Some(126 | 127))
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        pin::Pin,
        sync::{Arc, Mutex},
        task::{Context, Poll},
    };

    use tokio::io::AsyncWrite;

    use super::{ExecInput, is_missing_shell_status};

    /// Accepts at most three bytes per write, so the loop has to carry on.
    struct Trickle(Arc<Mutex<Vec<u8>>>, Arc<Mutex<bool>>);

    impl AsyncWrite for Trickle {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            let taken = bytes.len().min(3);
            self.0.lock().unwrap().extend_from_slice(&bytes[..taken]);
            Poll::Ready(Ok(taken))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            *self.1.lock().unwrap() = true;
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn input_writes_every_byte_and_closes() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(Mutex::new(false));
        let mut input = ExecInput::new(Box::pin(Trickle(written.clone(), closed.clone())));
        input.write(b"echo ok\r").await.unwrap();
        assert_eq!(written.lock().unwrap().as_slice(), b"echo ok\r");
        input.close().await;
        assert!(*closed.lock().unwrap());
    }

    #[test]
    fn only_126_and_127_mean_the_shell_is_missing() {
        assert!(is_missing_shell_status(Some(126)));
        assert!(is_missing_shell_status(Some(127)));
        assert!(!is_missing_shell_status(Some(0)));
        assert!(!is_missing_shell_status(Some(1)));
        assert!(!is_missing_shell_status(None));
    }
}
