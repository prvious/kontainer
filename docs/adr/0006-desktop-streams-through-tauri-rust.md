# 0006. Log and exec streams terminate in Tauri Rust and reach React over Channels

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/15-logs.md, docs/build/16-exec.md

## Context

The reset doc requires native Tauri networking: the WebView never talks to the controller. The v1 app
opened WebSockets from React with a token in the query string; that code is being deleted. Logs and
exec are binary WebSocket streams with protobuf frames (`kontainer-stream-protocol`,
`kontainer-stream-transport`). React needs a push channel for high-frequency data that `invoke`
request-response cannot provide.

## Decision

- The native client crate owns the WebSocket connection, authentication (Bearer header on upgrade),
  frame encoding, and reconnect policy. Tauri commands open a stream and return a stream id.
- Frames flow to React through a Tauri `Channel<StreamEvent>` passed in the open command. Events are
  small camelCase JSON objects: `line`, `gap`, `stdout`, `exit`, `error`, `closed`.
- Input flows the other way through ordinary commands: `exec_write`, `exec_resize`, `stream_close`.
- One stream registry in Tauri state maps stream ids to cancellation tokens. Closing the window or
  logging out cancels every stream.
- Limits from the transport crate stand: 64 KiB frames, four exec sessions per client.

## Consequences

- No WebSocket code in TypeScript. The mocked `invoke` and `Channel` in Vitest cover the UI.
- The Tauri crate's `tokio-tungstenite` dependency becomes used through the native client.
- Backpressure is the transport crate's bounded queue; when React falls behind, a `gap` event says
  how many lines were dropped, as the design's log states require.
