# 15. Logs

## Goal

Service log streaming over binary WebSocket from the manager engine, through the native client in
Tauri Rust, to a tabbed log viewer with follow, filter, reconnect, and gap handling.

Design: Logs `ZkgXL` (light `wbJWG`); states `RVrX9` (08.S3 reconnecting, gap, paused),
`f3Rq5` (08.S4 no lines and stream error); captions `uK0Bg`, `BKSgh`. Gaps G10 (stream picker),
G19 (load more history). Decisions: ADR 0006. Research: `docs/v2/logging.md`.

## Server

- Route `GET /api/v1/ws/logs`: WebSocket upgrade served by the `kontainer-stream-transport`
  server mounted in Axum. Authentication is the `Authorization: Bearer` header on the upgrade
  request, validated with the existing `validate_access` action; missing or invalid → 401 before
  upgrade. Expiry mid-stream closes with code 4401. No ticket fallback is built.
- Protocol (extend `kontainer-stream-protocol` additively, one payload per frame kind):
  - client → `LogsOpen { service_id, since_unix_ms?, tail (default 500, max 5000) }`; closing the
    socket ends the stream, and frame kind 4 (once `LogsClose`) is reserved (ADR 0040);
  - server → `LogLine { ts_unix_ms, stream (stdout | stderr), task_id, node_id, line, truncated }`
    (field 1, once `seq`, is reserved; the desktop numbers the lines it delivers),
    `LogGap { dropped }`, `LogEnd { reason (service_removed | error), code, message }`.
- Source: manager engine `service_logs(service_id, since, tail, follow, timestamps, details)`
  decoding the Docker multiplexed stream; task and node ids from the details prefix. One
  connection serves one service. Lines over 256 KiB are truncated with the flag set.
- Backpressure: the transport's bounded outbound queue; when the client falls behind, drop the
  oldest queued lines and send one `LogGap` with the count when the queue drains.
- Limits: eight open log connections per session; the ninth is refused with `limit_reached`.
- Never persist lines. Audit open and close events without content.

## Desktop (Tauri)

- Native client: `open_logs(service_id, since, tail) -> LogStream` over `tokio-tungstenite` with
  the frame codec; reconnect on drop with backoff 1 s doubling to 30 s, reopening with
  `since = last_ts − 1s` and dropping lines whose `ts` and content match ones already delivered in
  the overlap window; emits `reconnecting { attempt, next_retry_ms }`, `reconnected`, `gap`,
  `ended`.
- Commands: `logs_open(serviceId, since?, tail, channel) -> streamId`, `logs_close(streamId)`
  (cancels the follow, which closes the socket), `logs_retry_now(streamId)`. Channel events, batched every 25 ms:
  `{ type: "lines", lines: [{ seq, ts, stream, taskId, nodeId, text, truncated }] }`,
  `{ type: "gap", dropped }`, `{ type: "reconnecting", attempt, nextRetryMs }`,
  `{ type: "reconnected" }`, `{ type: "ended", reason, code, message }`.
- A stream registry in Tauri state with cancellation; logout and window close cancel all.

## Desktop (React)

- Route `/logs?service=` (search param validated; when present a tab for it opens on load).
- Header: `LOGS`, `// per-service log streams`, a filter input, NEW STREAM. NEW STREAM and the
  `+` tab open the stream picker (G10): a shadcn Command dialog listing services with search;
  choosing one opens a tab. The container row's Logs action navigates here with its service.
- Tabs (`cNE8S`): service name, dot (success live, warning reconnecting, destructive ended), close.
- Viewer: a buffer capped by the scrollback setting (default 10,000 lines) rendered with
  `@tanstack/react-virtual` (pre-approved). Columns TIMESTAMP `HH:mm:ss.SSS`, LEVEL badge parsed
  from the line with `\b(FATAL|ERROR|WARN(?:ING)?|INFO|DEBUG|TRACE)\b` (case-insensitive; no badge
  when absent; stderr lines without a level get `ERR` in destructive), MESSAGE in mono with a WRAP
  toggle. The filter input matches substrings (regex when wrapped in `/`), shows the match count,
  and applies to the buffer without re-requesting.
- Follow: at the bottom the view follows new lines; scrolling up pauses; a floating pill counts
  new lines with JUMP TO LATEST (08.S3). Derive "at bottom" from scroll events, not effects.
- Reconnecting (08.S3): tab dot amber, a warning Alert `Reconnecting… attempt 3 · next try in 4s`
  with RETRY NOW; lines stay; an inline gap row `N lines dropped while the client caught up`.
- No lines (08.S4): a filter with no matches shows an EmptyState repeating the query and the
  window (`last 500 lines since 14:02`) with CLEAR FILTER and LOAD PREVIOUS HOUR (G19: reopens the
  stream with `since = now − 1h` and `tail = 5000`). A stream that ended because the service was
  removed keeps its tab with a red dot; hovering explains; the tab body offers RESUME which reopens.
- Status bar `LOG STREAMS` counts open tabs.

## Tests

- Server: 401 before upgrade; open → lines with task and node ids from a fake engine stream;
  truncation flag; backpressure drop produces one gap with the right count; ninth connection
  refused; close on expiry.
- Native client: backoff sequence; overlap dedupe; ended propagation.
- Tauri: batching and registry cancellation on logout.
- React: level parsing table; follow and pill; reconnect alert with countdown; empty-filter state
  and load-previous-hour arguments; tabs open from the search param.
- Real cluster: stream a seed service, see new lines when hitting its endpoint through the manager
  DinD, remove the service, see the ended state, redeploy.

## Done when

- Two services stream side by side in tabs with live lines; pausing the manager container shows
  the reconnecting state and recovery; filters and follow behave as the captions say; all checks
  pass; `PROGRESS.md` updated; ADRs for the protocol messages and the virtualizer dependency.
