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
| 05 Membership and node metrics | done | Check-in with samples (container-scoped, disk every 12th, per-container net deltas), stats cache on `ListContainers`, `nodes`/`node_samples`/`controller_settings`, per-node agent clients + `fan_out`, `/nodes`, `/nodes/{id}/samples`, `/cluster/samples`, `/settings`, deep status node counts, hourly prune; static agent client removed (ADR 0015). Dev cluster: manager, worker-1, worker-2 `online` with CPU/memory/disk; `24h/15m` → 96 buckets (95 null before the agents started); `3 of 3 nodes online`; killing worker-2 → `degraded` at 22 s, `2 of 3`, then `offline`. Checks: server fmt/clippy/test (Postgres), Tauri fmt/clippy/test, app test/types/lint/fmt | see next |
| 06 Operations | todo | | |
| 07 Containers | todo | | |
| 08 Images | todo | | |
| 09 Volumes | todo | | |
| 10 Networks | todo | | |
| 11 Services | todo | | |
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
