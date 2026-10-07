# 0043. Idempotent start and stop, Docker times through chrono, and the agent RPC's last unread fields

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/07-containers.md, docs/build/08-images.md, docs/build/10-networks.md
- Amends: ADR 0017, ADR 0040
- Decision record: PR 135 review items PT-51 and PT-65

## Context

- **`already_in_state` never happens.** ADR 0017 maps Docker's 304 to an engine `NotModified` kind,
  the agent status `already_in_state`, and a controller failure "The container was already
  running.". bollard treats 304 as success (`docker.rs`, `s.is_success() || s == NOT_MODIFIED`), so
  starting a running container or stopping a stopped one already returned `Ok(())`. The mapping, its
  bulk failure path, and the tests that proved it with fakes returning `NotModified` described
  behaviour that doesn't exist.
- **The hand-written date parser.** ADR 0017 parses Docker's RFC 3339 times without a date library.
  That came to about 60 lines across `parse_docker_time` and the service-log `timestamp_millis`. The
  workspace already depends on chrono.
- **Unread agent RPC fields.** ADR 0040 dropped `dangling`, `ingress` and `internal` from the
  controller's rows but left the agent RPC and the engine carrying them as far as the RPC client.
  `InspectNetworkResponse.network` was also parsed and validated, and then nothing read it.

## Decision

- **Start and stop are idempotent.** Starting a running container or stopping a stopped one
  succeeds, as Docker already makes it. `EngineErrorKind::NotModified`, the agent's
  `already_in_state` status, `AgentClientError::AlreadyInState` and the controller's
  `already_in_state` failure code are removed. A bulk action never reports a container as failed
  for already being in the requested state.
- **Docker times are parsed with chrono.** `parse_docker_time` and service-log timestamps use
  `DateTime::parse_from_rfc3339`. Docker's "never" (any year before 1970) and unparsable text still
  read as no time. Sub-millisecond digits are truncated, and offsets are honoured.
- **The agent RPC stops carrying unread data.**
  - `ImageSummary.dangling` (field 7) is reserved.
  - `NetworkSummary.ingress` and `internal` (fields 10 and 11) are reserved. Built-in networks are
    matched by name.
  - `InspectNetworkResponse.network` (field 1) is reserved. The controller asks only for the
    endpoints.
  - The engine's `ImageSummary.dangling` and `NetworkSummary.ingress`/`internal` are removed.

No public HTTP response changes.

## Consequences

- An older agent that still sends `already_in_state` is read as an unexpected answer
  (`invalid_response`). No released agent sends it, because bollard never surfaced the 304.
- Spec 07 still lists `already_in_state` among the agent's codes and in its agent test. That line
  is stale.
