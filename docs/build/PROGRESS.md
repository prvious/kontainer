# Progress

Update this file before every commit. Status is one of `todo`, `in-progress`, `done`, `blocked`.
A `blocked` row carries what was tried and the exact error. A `done` row names the commit that
finished it and the checks that passed.

| Spec | Status | Note | Commit |
|------|--------|------|--------|
| 01 Cleanup | done | v1 client deleted, scaffold renamed, tests type-checked, dead-code guard; `/` redirects to `/tasks` until spec 14; CodeMirror removed (ADR 0011). Checks: app test/types/lint/fmt, Tauri fmt/test | 7f47279 |
| 02 Dev environment | done | `DockerHost` tcp override gated by env (ADR 0012), dev/pki.sh, agent launchers, smoke.sh passes (ready + 3 mTLS handshakes), DinD IPs pinned. Deep-status curl is recorded under spec 04. Checks: server fmt/clippy/test (Postgres), dev-cluster engine tests | 6853145 |
| 03 Shell and foundations | done | Design tokens (light/dark), chrome (titlebar, sidebar, status bar, splash + 02.S1), kit, DataTable on TanStack Table v9, Recharts primitives (ADR 0013). Checks: app test/types/lint/fmt, Tauri fmt/test | see next |
| 04 Auth and connect | todo | | |
| 05 Membership and node metrics | todo | | |
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
