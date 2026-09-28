# Progress

Update this file before every commit. Status is one of `todo`, `in-progress`, `done`, `blocked`.
A `blocked` row carries what was tried and the exact error. A `done` row names the commit that
finished it and the checks that passed.

| Spec | Status | Note | Commit |
|------|--------|------|--------|
| 01 Cleanup | done | v1 client deleted, scaffold renamed, tests type-checked, dead-code guard; `/` redirects to `/tasks` until spec 14; CodeMirror removed (ADR 0011). Checks: app test/types/lint/fmt, Tauri fmt/test | 7f47279 |
| 02 Dev environment | done | `DockerHost` tcp override gated by env (ADR 0012), dev/pki.sh, agent launchers, smoke.sh passes (ready + 3 mTLS handshakes), DinD IPs pinned. Deep status with a password session: `{"build":{"version":"0.1.0"},"migration":"current","database":"reachable","agentRpc":"up"}`. Checks: server fmt/clippy/test (Postgres), dev-cluster engine tests | 6853145 |
| 03 Shell and foundations | done | Design tokens (light/dark), chrome (titlebar, sidebar, status bar, splash + 02.S1), kit, DataTable on TanStack Table v9, Recharts primitives (ADR 0013). Checks: app test/types/lint/fmt, Tauri fmt/test | 1d65183 |
| 04 Auth and connect | done | Owner registration, password sign-in, throttle (10/15 min), `auth:reset-password email:<email>`, Connect card (02.S2, 02.S7, lock), saved connections, switcher (02.S6), session-expired dialog (02.S5), health dot (ADR 0014). Verified on the dev controller with curl: register 201 → 409, login 200, wrong 401 `attemptsRemaining` 9…1, 10th 429, reset → new password 200. Checks: server fmt/clippy/test (Postgres), Tauri fmt/clippy/test, app test/types/lint/fmt | 24fb1a2 |
| 05 Membership and node metrics | done | Check-in with samples (container-scoped, disk every 12th, per-container net deltas), stats cache on `ListContainers`, `nodes`/`node_samples`/`controller_settings`, per-node agent clients + `fan_out`, `/nodes`, `/nodes/{id}/samples`, `/cluster/samples`, `/settings`, deep status node counts, hourly prune; static agent client removed (ADR 0015). Dev cluster: manager, worker-1, worker-2 `online` with CPU/memory/disk; `24h/15m` → 96 buckets (95 null before the agents started); `3 of 3 nodes online`; killing worker-2 → `degraded` at 22 s, `2 of 3`, then `offline`. Checks: server fmt/clippy/test (Postgres), Tauri fmt/clippy/test, app test/types/lint/fmt | becc7ee |
| 06 Operations | done | Kinds + timeouts, `Idempotency-Key` admission (202/409), worker with recovery, bounded concurrency, timeout/panic/unsupported codes, hourly prune; finalize, cancel, progress, events routes; bulk runner with `partial_failure`; desktop `useOperation`, `OperationsProvider`, `useStartOperation`, pinned Operation Status rows, toasts (ADR 0016). Dev controller: `noop` admitted 202 → replay 202 `replayed: true`, missing key 400, progress 1…3 of 3 → `succeeded`; `kill -9` mid-run → restart shows `uncertain` / `recovered_after_restart`. The desktop row and toast are verified by component tests (no screen capture here). Checks: server fmt/clippy/test (Postgres), Tauri fmt/clippy/test, app test/types/lint/fmt | 309627f |
| 07 Containers | done | Agent `ContainerAction` + inspect-time cache + ports/labels; `GET /api/v1/containers` fan-out with outcomes and the 2,000 cap (413); action and bulk admission + executors; `/containers` page with states strip, URL filters/sort, bulk bar with typed confirms, partial results (UNKNOWN rows), empty states, remove rule (ADR 0017). Dev cluster: 15 seed containers across 3 nodes with CPU/memory; restart, stop (3 s grace) → exited, start, bulk pause 2 → paused, unpause; stopping worker-2's agent → `unavailable` outcome in 0.1 s with the other 12 rows. Exec/Logs entries stay disabled until specs 15/16. UI verified by component tests. Checks: server fmt/clippy/test (Postgres), Tauri fmt/clippy/test, app test/types/lint/fmt | 27b5d88 |
| 08 Images | done | Agent `ListImages`/`RemoveImage`/streaming `PullImage` behind the engine trait; `GET /api/v1/images` merged by id, `DELETE /api/v1/images/{id}` and `POST /api/v1/images/pulls` with Docker-grammar reference validation (400 field errors); `image.remove` (concurrency 4, skips) and `image.pull` (parallel, per-node detail, verbatim registry errors) executors; `/images` page with storage strip, search/sort, typed delete when in use, pull dialog with per-node bars, RETRY, and RUN IN BACKGROUND toast (ADR 0018). Dev cluster: pulled `alpine:3.20` on all three nodes (3/3 layers each), listed as one row on 3 nodes, removed from worker-2 only, then from all; deleting in-use `traefik/whoami` without force failed with "Containers use this image; delete it with force…"; `Alpine` → 400 `invalid_name`. Found and fixed: containerd-store tags carry `@sha256:` (display/confirm as `name:tag`) and finished pulls reported 1/3 layers. UI verified by component tests. Checks: server fmt/clippy/test (Postgres), native client, Tauri fmt/clippy/test, app test/types/lint/fmt | 7a9d610 |
| 09 Volumes | done | Agent `ListVolumes`/`CreateVolume`/`RemoveVolume` behind the engine trait, sizes from a verbose `df` cached every twelfth sample, in-use refusal as `in_use_by` names; `GET /api/v1/volumes[?node=&name=]` per node, `POST`/`DELETE` admission with Docker's name rule (2–255, ADR 0019) and `local` only; `volume.create`/`volume.remove` executors; `/volumes` page with storage strip, node/driver/attached filters, size/name/created sorts, create dialog (node select, blur conflict query, labels), 3 s `highlightedRow`, disabled trash with users for attached volumes, typed name to remove. Dev cluster: seed stacks now carry `toolbox_redis-data` and a global `monitoring_collector-cache`, listed attached on their nodes with sizes; created `verify-data` on worker-1 (driver local), duplicate refused, removing the mounted redis volume failed "In use by toolbox_redis.1…", removal worked; name `a` and driver `nfs` → 400 fields. Engine dev-cluster test covers the Docker behaviours. UI verified by component tests. Checks: server fmt/clippy/test (Postgres), native client, Tauri fmt/clippy/test, app test/types/lint/fmt | a80e2eb |
| 10 Networks | done | Controller `ManagerEngine` port (Docker manager via `connect_for_manager`, describe-gated, 503 `not_a_manager` otherwise) passed to executors; engine and agent `ListNetworks`/`InspectNetwork`/`CreateNetwork`/`RemoveNetwork` with built-in refusal and `in_use_by` names; `GET /api/v1/networks` merging manager swarm rows with per-node endpoint counts and local rows per node, inspect across nodes, create (overlay on manager, bridge on a node, uniqueness and CIDR checks) and remove (built-in 400, `in_use` with names); container DTO `networks` end to end with a Containers `network` filter (ADR 0020); `/networks` page with overview strip, filters, blocked-remove popover linking to containers, plain remove for empty networks, create dialog with overlay/bridge fields and 3 s highlight. Dev cluster: 16 rows (built-ins per node, `ingress` swarm), seed overlays `toolbox_kontainer` 5 / `webapp_kontainer` 6 / `monitoring_kontainer` 4 containers across 3 nodes; inspect named each container and node; created an attachable overlay (swarm, 10.231.5.0/24) and a bridge on worker-2; removing `ingress` → 400 `builtin_network`; removing `toolbox_kontainer` failed `in_use` with the five task names; both test networks removed. Engine dev-cluster test covers the Docker side. UI verified by component tests. Checks: server fmt/clippy/test (Postgres), native client, Tauri fmt/clippy/test, app test/types/lint/fmt | 35f8e7e |
| 11 Services | done | Engine and `ManagerEngine` services/tasks with versioned scale, force update, and rollback (`VersionConflict`), quorum wording → `QuorumUnavailable`; `GET /api/v1/services` (one tasks call), `GET /api/v1/services/{id}` (tasks + newest error), `POST …/actions/{scale|restart|rollback}` with 503 `quorum_unavailable` + `retryAfterSeconds` before admission and 409 `version_conflict`; convergence executor (2 s polls, 60 s, `converge_timeout` with the task error); native/IPC `quorum_unavailable`, `not_a_manager`, `version_conflict`; `/services` page with derived status, inline stepper, SCALING track, partial tooltip, restart/rollback confirms, quorum error state with countdown (ADR 0021). Dev cluster: 8 seed services listed with correct counts; `webapp_worker` 1 → 2 converged, stale version 409, 2 → 1 converged, restart converged on a fresh task; after `docker service update --image nginx:alpine` via the CLI, rollback returned it to `traefik/whoami` (`rollback_completed`); `docker pause kontainer-cluster-manager-1` → list and restart 503 `quorum_unavailable` in 3 s, recovered after unpause. Logs menu entry disabled until spec 15. UI verified by component tests. Checks: server fmt/clippy/test (Postgres), native client, Tauri fmt/clippy/test, app test/types/lint/fmt | see next |
| 12 Stacks | todo | | |
| 13 Nodes | todo | | |
| 14 Overview | todo | | |
| 15 Logs | todo | | |
| 16 Exec | todo | | |
| 17 Settings | todo | | |
| 18 Integrations | todo | | |

## Log

Append one line per work session: date, spec, what happened, what is next.

- 2026-09-28 — plan written; nothing implemented yet.
- 2026-09-28 — spec 01 done. Spec 02: engine TCP host, agent/controller env gates, dev PKI verified by a
  real `Describe` (`KONTAINER_TEST_DEV_PKI_DIR=../dev/pki`), dev-cluster engine tests green. The DinD
  cluster was reset because Docker reassigned bridge IPs and stranded the workers; node IPs are now
  fixed. The owner's local `kontainer_development` database carries a stale migration
  (`m20260907_000001_create_sessions`) from an abandoned schema and the controller refuses to start
  on it; verification runs use `DATABASE_URL=…/kontainer_build_verification` instead. To reset it:
  `docker exec kontainer-server-postgres-1 psql -U kontainer -d postgres -c 'DROP DATABASE kontainer_development' -c 'CREATE DATABASE kontainer_development'`.
  Next: spec 03.
- 2026-09-28 — spec 03 done (1d65183). Spec 04: server password auth + throttle + reset task, native
  client auth calls with request ids and probes, Tauri custody on password sign-in with saved
  connections, React Connect card, switcher, and session-expired dialog. Screen capture is not
  permitted in this environment, so the desktop UI is verified by tests; the controller flow was
  verified with curl against the dev cluster. Next: spec 05.
- 2026-09-28 — spec 04 done (24fb1a2). Spec 05: agent sampler and supervised check-in, controller
  membership, samples, settings, fan-out, and desktop node/sample/settings commands; the status bar
  and sidebar read membership. Verified live on the dev cluster (three agents, status transitions,
  bucket nulls). Next: spec 06.
- 2026-09-28 — spec 05 done (becc7ee). Spec 06: durable operations end to end (admission edge,
  worker, events, cancel, prune, bulk runner) and the desktop operation model; verified on the dev
  controller including crash recovery. Next: spec 07.
- 2026-09-28 — spec 06 done (309627f). Spec 07: cluster containers end to end; found and fixed a
  2 s channel timeout that failed long stops (regression test over real mTLS). Starting stopped
  Swarm task containers during testing left orphans in the dev cluster; they were stopped again.
  Next: spec 08.
- 2026-09-28 — spec 07 done (27b5d88). Spec 08: images end to end — list, removal, and pulls with
  per-node progress; verified pull/remove/in-use refusal on the dev cluster, which is back to its
  seed images. Next: spec 09.
- 2026-09-28 — spec 08 done (7a9d610). Spec 09: volumes end to end; removal needs the typed name
  (security floor over the spec's plain dialog), names need two characters (Docker's rule). Seed
  stacks now declare volumes. Next: spec 10.
- 2026-09-28 — spec 09 done (a80e2eb). Spec 10: networks end to end and the controller's manager
  engine; swarm inspect asks the nodes (a manager only sees its own endpoints). Next: spec 11.
- 2026-09-28 — spec 10 done (35f8e7e). Spec 11: services end to end with convergence tracking and
  the quorum state; the dev cluster's `webapp_worker` was scaled, restarted, and rolled back and is
  back to 1 replica on `traefik/whoami`. Next: spec 12.
