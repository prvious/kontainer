use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use kontainer_stream_protocol::{StreamFrame, StreamProtocolError, WEBSOCKET_SUBPROTOCOL};
use tokio::{net::TcpStream, time};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{
            HeaderValue,
            header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL},
        },
        protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
    },
};
use tokio_util::sync::CancellationToken;

use crate::{StreamClose, StreamLimits};

type NativeSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct NativeStreamClient {
    socket: NativeSocket,
    limits: StreamLimits,
}

impl NativeStreamClient {
    pub async fn connect(
        endpoint: &str,
        limits: StreamLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, StreamClientError> {
        Self::connect_with_bearer(endpoint, None, limits, cancellation).await
    }

    /// Connect sending `Authorization: Bearer <credential>` on the upgrade request. The
    /// credential never appears in errors.
    pub async fn connect_with_bearer(
        endpoint: &str,
        bearer: Option<&str>,
        limits: StreamLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, StreamClientError> {
        let mut request = endpoint
            .into_client_request()
            .map_err(|_| StreamClientError::InvalidEndpoint)?;
        if let Some(bearer) = bearer {
            let mut value = HeaderValue::from_str(&format!("Bearer {bearer}"))
                .map_err(|_| StreamClientError::InvalidEndpoint)?;
            value.set_sensitive(true);
            request.headers_mut().insert(AUTHORIZATION, value);
        }
        request.headers_mut().insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(WEBSOCKET_SUBPROTOCOL),
        );
        let configuration = WebSocketConfig::default()
            .write_buffer_size(0)
            .max_write_buffer_size(limits.maximum_queued_bytes())
            .max_message_size(Some(limits.maximum_frame_bytes()))
            .max_frame_size(Some(limits.maximum_frame_bytes()));

        let connection = connect_async_with_config(request, Some(configuration), false);
        let (socket, response) = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(StreamClientError::Cancelled),
            result = connection => result.map_err(map_connection_error)?,
        };

        let selected_protocol = response
            .headers()
            .get(SEC_WEBSOCKET_PROTOCOL)
            .and_then(|value| value.to_str().ok());
        if selected_protocol != Some(WEBSOCKET_SUBPROTOCOL) {
            return Err(StreamClientError::SubprotocolMismatch);
        }

        Ok(Self { socket, limits })
    }

    pub async fn send_frame(
        &mut self,
        frame: &StreamFrame,
        cancellation: &CancellationToken,
    ) -> Result<(), StreamClientError> {
        let encoded = frame.encode();
        if encoded.len() > self.limits.maximum_frame_bytes() {
            return Err(StreamClientError::FrameTooLarge);
        }

        tokio::select! {
            biased;
            () = cancellation.cancelled() => {
                self.close_with_handshake(StreamClose::CLIENT_CANCELLED).await;
                Err(StreamClientError::Cancelled)
            }
            result = self.socket.send(Message::Binary(encoded.into())) => {
                result.map_err(|_| StreamClientError::TransportFailure)
            }
        }
    }

    pub async fn receive_frame(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<StreamFrame, StreamClientError> {
        loop {
            let message = tokio::select! {
                biased;
                () = cancellation.cancelled() => {
                    self.close_with_handshake(StreamClose::CLIENT_CANCELLED).await;
                    return Err(StreamClientError::Cancelled);
                }
                message = self.socket.next() => message,
            };

            match message {
                Some(Ok(Message::Binary(bytes))) => {
                    if bytes.len() > self.limits.maximum_frame_bytes() {
                        self.close_with_handshake(StreamClose::MESSAGE_TOO_LARGE)
                            .await;
                        return Err(StreamClientError::FrameTooLarge);
                    }
                    return match StreamFrame::decode(&bytes) {
                        Ok(frame) => Ok(frame),
                        Err(error) => {
                            self.close_with_handshake(StreamClose::for_envelope_error(error))
                                .await;
                            Err(StreamClientError::Protocol(error))
                        }
                    };
                }
                Some(Ok(Message::Close(frame))) => {
                    // Tungstenite queues the peer's acknowledgement when reading Close.
                    let _ = time::timeout(self.limits.shutdown_grace(), self.socket.flush()).await;
                    let (code, reason) = frame
                        .map(|frame| (u16::from(frame.code), frame.reason.to_string()))
                        .unwrap_or((1005, String::new()));
                    return Err(StreamClientError::Closed { code, reason });
                }
                Some(Ok(Message::Text(_))) => {
                    self.close_with_handshake(StreamClose::BINARY_REQUIRED)
                        .await;
                    return Err(StreamClientError::UnexpectedTextFrame);
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Err(error)) => {
                    let close = StreamClose::for_transport_error(&error);
                    if let Some(close) = close {
                        self.close_with_handshake(close).await;
                    }
                    return Err(if close == Some(StreamClose::MESSAGE_TOO_LARGE) {
                        StreamClientError::FrameTooLarge
                    } else {
                        StreamClientError::TransportFailure
                    });
                }
                None => return Err(StreamClientError::ConnectionEnded),
            }
        }
    }

    pub async fn round_trip(
        &mut self,
        frame: &StreamFrame,
        cancellation: &CancellationToken,
    ) -> Result<StreamFrame, StreamClientError> {
        self.send_frame(frame, cancellation).await?;
        self.receive_frame(cancellation).await
    }

    pub async fn close(&mut self) {
        self.close_with_handshake(StreamClose::NORMAL).await;
    }

    /// Separate halves so one task can read while another writes, as a terminal does.
    #[must_use]
    pub fn split(self) -> (NativeStreamSender, NativeStreamReceiver) {
        let (sink, stream) = self.socket.split();
        (
            NativeStreamSender {
                sink,
                limits: self.limits,
            },
            NativeStreamReceiver {
                stream,
                limits: self.limits,
            },
        )
    }

    async fn close_with_handshake(&mut self, close: StreamClose) {
        let message = Message::Close(Some(CloseFrame {
            code: CloseCode::from(close.code),
            reason: close.reason.into(),
        }));
        let _ = time::timeout(self.limits.shutdown_grace(), async {
            if self.socket.send(message).await.is_err() {
                return;
            }
            loop {
                match self.socket.next().await {
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                    Some(Ok(_)) => {}
                }
            }
        })
        .await;
    }
}

/// The writing half of a split client.
pub struct NativeStreamSender {
    sink: SplitSink<NativeSocket, Message>,
    limits: StreamLimits,
}

impl NativeStreamSender {
    pub async fn send_frame(
        &mut self,
        frame: &StreamFrame,
        cancellation: &CancellationToken,
    ) -> Result<(), StreamClientError> {
        let encoded = frame.encode();
        if encoded.len() > self.limits.maximum_frame_bytes() {
            return Err(StreamClientError::FrameTooLarge);
        }
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(StreamClientError::Cancelled),
            result = self.sink.send(Message::Binary(encoded.into())) => {
                result.map_err(|_| StreamClientError::TransportFailure)
            }
        }
    }

    /// Send a normal close; the receiving half sees the peer's answer.
    pub async fn close(&mut self) {
        let message = Message::Close(Some(CloseFrame {
            code: CloseCode::from(StreamClose::NORMAL.code),
            reason: StreamClose::NORMAL.reason.into(),
        }));
        let _ = time::timeout(self.limits.shutdown_grace(), self.sink.send(message)).await;
    }
}

/// The reading half of a split client. It cannot answer a bad frame with a close itself, so it
/// reports it and the owner closes through the sender.
pub struct NativeStreamReceiver {
    stream: SplitStream<NativeSocket>,
    limits: StreamLimits,
}

impl NativeStreamReceiver {
    pub async fn receive_frame(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<StreamFrame, StreamClientError> {
        loop {
            let message = tokio::select! {
                biased;
                () = cancellation.cancelled() => return Err(StreamClientError::Cancelled),
                message = self.stream.next() => message,
            };
            match message {
                Some(Ok(Message::Binary(bytes))) => {
                    if bytes.len() > self.limits.maximum_frame_bytes() {
                        return Err(StreamClientError::FrameTooLarge);
                    }
                    return StreamFrame::decode(&bytes).map_err(StreamClientError::Protocol);
                }
                Some(Ok(Message::Close(frame))) => {
                    let (code, reason) = frame
                        .map(|frame| (u16::from(frame.code), frame.reason.to_string()))
                        .unwrap_or((1005, String::new()));
                    return Err(StreamClientError::Closed { code, reason });
                }
                Some(Ok(Message::Text(_))) => return Err(StreamClientError::UnexpectedTextFrame),
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Err(_)) => return Err(StreamClientError::TransportFailure),
                None => return Err(StreamClientError::ConnectionEnded),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamClientError {
    InvalidEndpoint,
    ConnectionFailed,
    HandshakeRejected { status: u16 },
    SubprotocolMismatch,
    FrameTooLarge,
    Protocol(StreamProtocolError),
    UnexpectedTextFrame,
    Closed { code: u16, reason: String },
    ConnectionEnded,
    TransportFailure,
    Cancelled,
}

fn map_connection_error(error: tokio_tungstenite::tungstenite::Error) -> StreamClientError {
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            StreamClientError::HandshakeRejected {
                status: response.status().as_u16(),
            }
        }
        _ => StreamClientError::ConnectionFailed,
    }
}
