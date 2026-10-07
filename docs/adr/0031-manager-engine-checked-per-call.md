# 0031. Check the manager engine's Swarm role per call, not once at boot

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/10-networks.md
- Amends: ADR 0020, ADR 0022

## Context

ADR 0020 had the controller call `describe()` once at `serve` and keep the manager engine only if
the configured Docker host was a Swarm manager. If Docker was down, slow or locked at that moment,
the engine stayed absent until the controller restarted. Every manager-scoped feature (overlay
networks, services, stacks, logs, Swarm node state) then answered `not_a_manager`, and the message
blamed the configuration. The operation worker kept its own copy of that startup decision.

## Decision

- **Always build the engine.** `connect_manager` builds `DockerManagerEngine` from
  `settings.docker.host` without asking Docker anything. It returns `None` only when the engine
  crate rejects the host configuration. Routes and the operation worker share that one engine.
  `ManagerDockerHost` is gone: `serve` reads the host from the settings it already loads, and
  `init_docker_state` only validates it.
- **Check the role per call.** Each `DockerManagerEngine` call first needs a `describe()` answer
  that is less than 30 s old.
  - **Not a manager** (a worker, or not in a Swarm, pending or locked): the call fails
    `ManagerError::NotAManager` without reaching Docker. A worker answers `network ls` with only
    its local view, and that view must not be presented as the cluster's.
  - **`describe()` fails** (daemon down, or no answer within 5 s): nothing is cached. The call fails
    as unavailable, and the next call asks again.
  - A change of answer to "not a manager" is logged once.
- **Demotion inside the 30 s.** `classify_bollard_error` maps Docker's "not a swarm manager"
  refusal to `EngineErrorKind::NotAManager`, which becomes `ManagerError::NotAManager`.
- **Responses.** `NotAManager` answers 503 `not_a_manager` wherever it surfaces:
  - service and stack reads and admissions (`manager_read_error`);
  - the log stream upgrade;
  - the network list's `manager` field;
  - operation failures (`manager_failure`).
  Network create and remove now read `not_a_manager` from the manager status their list call
  already returns, instead of from whether the engine exists.

## Also corrected: Docker's task filter 404 (ADR 0022)

ADR 0022 read Docker's 404 on a service-filtered task list as "the services' tasks are gone".
Docker actually answers 404 as soon as the filter names any service that no longer exists, even
while that service's tasks are still stopping. So the stack removal wait ended on its first poll,
and a service list taken during a removal failed with 503.

Both places now list tasks once without a filter and group them by `service_id` in Rust:
- The stack removal waits until no task of the removed services is in a state other than
  `complete`, `shutdown`, `failed`, `rejected`, `orphaned` or `remove`.
- The test fakes now model the 404 the way Docker does.

## Consequences

- A controller started before Docker, or before the host joined or was promoted in the Swarm,
  works within 30 s without a restart.
- Each manager call can add one `describe()` (ping, version, info) every 30 s, and one per call
  while the daemon is down.
- On a worker-hosted controller the manager engine now exists, so code that only checks whether
  the engine is present no longer detects the worker. Such code has to handle `NotAManager`
  instead.
- The unfiltered task list grows with the whole Swarm's task history, not one stack's. That is
  fine at this product's cluster sizes.
