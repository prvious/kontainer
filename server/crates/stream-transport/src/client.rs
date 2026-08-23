use futures_util::{SinkExt, StreamExt};
use kontainer_stream_protocol::{StreamFrame, StreamProtocolError, WEBSOCKET_SUBPROTOCOL};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderValue, header::SEC_WEBSOCKET_PROTOCOL},
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
        let mut request = endpoint
            .into_client_request()
            .map_err(|_| StreamClientError::InvalidEndpoint)?;
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
                self.send_close(StreamClose::CLIENT_CANCELLED).await;
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
                    self.send_close(StreamClose::CLIENT_CANCELLED).await;
                    return Err(StreamClientError::Cancelled);
                }
                message = self.socket.next() => message,
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

    pub async fn round_trip(
        &mut self,
        frame: &StreamFrame,
        cancellation: &CancellationToken,
    ) -> Result<StreamFrame, StreamClientError> {
        self.send_frame(frame, cancellation).await?;
        self.receive_frame(cancellation).await
    }

    pub async fn close(&mut self) {
        self.send_close(StreamClose::NORMAL).await;
    }

    async fn send_close(&mut self, close: StreamClose) {
        let _ = self
            .socket
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::from(close.code),
                reason: close.reason.into(),
            })))
            .await;
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
