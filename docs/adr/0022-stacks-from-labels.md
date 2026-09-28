# 0022. Stacks from service labels, and stack removal

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/12-stacks.md

## Context

Spec 12 derives stacks from `com.docker.stack.namespace`, previews what removing one takes away,
and removes it as a durable operation that keeps volumes. Swarm has no stack object, so the
controller composes one from services, their tasks, their containers, and labelled networks.

## Decision

- **Engine and port.** `ServiceSummary` gains `volumes`: the named-volume sources of its mounts
  (`type: volume`). `ContainerEngine` and `ManagerEngine` gain `remove_service`.
- **List.** `GET /api/v1/stacks` groups the manager's services by stack label, with running and
  desired counts from the one tasks call spec 11 already makes.
  - CPU and memory are summed from the containers fan-out (spec 07) by the containers' stack
    label. When that fan-out fails, the figures are left out, not the stacks.
  - `lastDeployed` is the newest service update. Status is `stopped` when no task runs,
    `partial` when any service is short, otherwise `running`.
  - Docker keeps no Compose file, so rows have no file subtitle.
- **Plan.** `GET /api/v1/stacks/{name}/plan` lists the stack's services, the networks carrying its
  label, and the named volumes its services mount as `volumesRetained`. An unknown stack is 404.
  The same read runs before admitting a removal, so a manager without quorum admits nothing
  (503 `quorum_unavailable`, as in ADR 0021).
- **Removal executor.** `stack.remove` removes every service, then polls tasks every 2 s for up to
  60 s. Docker answers 404 for a task filter that names only removed services, and that counts as
  gone: the first live run waited the full minute before this was handled. It then removes each
  labelled network on the manager, retrying while Swarm still detaches endpoints, for up to 60 s.
  - Volumes are never touched.
  - Detail is `{ phase: removing_services | waiting_for_tasks | removing_networks | removed,
    servicesRemoved, networksRemoved, volumesRetained }`.
  - The kind's timeout is 150 s.
  - A timeout fails `tasks_remaining` or `in_use` and says what was already removed.
- **Desktop.**
  - `/stacks` has the empty state (toolbar hidden, a DOCKER DOCS button that opens the stack
    deploy page through the opener plugin) and DEPLOY STACK disabled (G3).
  - The row menu has Show services, which navigates to `/services?stack=<name>` (G9), and Remove.
  - The remove dialog loads the plan first, with the removed items and the retained volumes in
    separate boxes. Typing the stack name enables REMOVE STACK. There is no re-authentication
    (ADR 0002).
  - The dialog shows the running phase and closes on success. The toast reports counts, and
    operation summaries learned to read the removal detail.

## Consequences

- Removing a stack leaves its volumes. Redeploying the same stack reuses them, which the dev
  cluster showed with `toolbox_redis-data`.
- Swarm removes an overlay asynchronously, so `docker network ls` can still show it for a moment
  after the operation succeeds.
