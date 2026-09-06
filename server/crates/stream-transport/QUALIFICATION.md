# Binary WebSocket qualification

This foundation slice qualifies the shared public stream framing boundary. It
does not add a controller route or implement log, exec, authentication-ticket,
or Docker payload semantics. The Axum route and acknowledgement application
used by the tests exist only in the loopback fixture.

## Contract and limits

- Subprotocol: `kontainer.stream.v1`
- Encoding: one binary WebSocket message containing one protobuf
  `kontainer.stream.v1.StreamEnvelope`
- Stable data fixture (sequence 7, payload `ping`):
  `[8, 1, 16, 7, 24, 1, 34, 4, 112, 105, 110, 103]`
- Maximum message and frame: 64 KiB
- Maximum outbound queue: 32 frames and 1 MiB total encoded bytes
- Maximum concurrent sessions per configured server seam: 4
- Shutdown grace: 2 seconds

Fallible configuration may lower those values but cannot exceed compiled safety
ceilings of 64 KiB per frame, 1,024 queued frames, 16 MiB queued bytes, or
1,024 concurrent sessions. This prevents pathological configuration from
reaching Tokio channel or semaphore constructors.

| Rejection | WebSocket close |
|---|---|
| Text message | `1003 binary protobuf required` |
| Unknown protocol version | `1002 unsupported protocol version` |
| Unknown frame kind | `1002 unknown frame kind` |
| Malformed WebSocket framing | `1002 websocket protocol violation` |
| Invalid WebSocket UTF-8 | `1007 invalid websocket utf-8` |
| Malformed protobuf | `1007 malformed protobuf envelope` |
| Oversized message/frame | `1009 frame exceeds configured limit` |
| Queue item/byte overload | `1013 slow consumer exceeded queue limits` |
| Server cancellation | `1001 server shutdown` |

Every admitted session owns one reader and one writer task. The session
supervisor awaits both join handles; normal completion increments a separate
completed counter, while aborts or join failures remain visible. Qualification
fails unless started and completed task counts match, active tasks reach zero,
and failed joins remain zero.

The loopback suite sends one small trigger, then stops reading from the real
socket while a test application yields between 60 KiB outbound frames. The
server writer eventually backs up and the bounded application queue rejects
further output without any unread inbound flood that could turn the intended
close into a TCP reset. Client-originated cancellation must complete its close
handshake and drain the server session before server shutdown is requested.
Malformed, unknown, text, and oversized server fixtures likewise assert the
native client's exact rejection close code and reason.
Peer-initiated closes are acknowledged before the native client returns, and
client-initiated close sending and acknowledgement share one shutdown deadline.

## Reproducible evidence

The `Stream qualification` workflow records the commit, runner architecture,
Rust version, resolved Axum/Tokio/Hyper/rustls/tokio-tungstenite graph, command,
frame fixture, effective limits, close codes/reasons, lifecycle counters, and
raw test output in the uploaded `stream-qualification` artifact.

Run the same loopback suite locally with:

```shell
cargo test --manifest-path server/Cargo.toml \
  --package kontainer-stream-transport --test loopback -- --nocapture
```

The native client enables tokio-tungstenite's rustls/WebPKI-root path for WSS.
The loopback fixture intentionally uses cleartext localhost WebSocket so this
slice isolates framing and lifecycle; real TLS identity is qualified by #58.
Results on the current ARM64 macOS host are not Linux AMD64 or ARM64 release
evidence, which remains tracked by #60.
