# 01. Cleanup

## Goal

Remove every remnant of the v1 Go/Goravel client so nothing in the next seventeen specs is built on
dead code, rename the Tauri scaffold, and leave all checks green. Deletion only; no new features.
Git history keeps everything, so delete rather than comment out or "keep for reference".

## What stays (the live path)

- Tauri commands: `session_login`, `session_cancel_login`, `session_restore`, `session_retry_commit`,
  `session_logout`, `session_login_hint`, `get_configured_node`, `list_container_inventory`,
  `close_splashscreen`, `load_app_settings`, `save_app_settings`, and the `session/` module that
  backs them. Spec 04 reshapes the login commands; do not touch them here.
- React: `__root.tsx`, `login.tsx`, `_authenticated.tsx`, `tasks/index.tsx`, `settings/index.tsx`;
  components `login-screen`, `authenticating-screen`, `session-issue`, `cluster-switcher`,
  `splash-screen`, `accent-line`, `logo`, `page-header`, `settings/*`; contexts `auth`, `settings`,
  `appearance`; hooks `session/*`, `use-container-inventory`, `use-auth`, `use-settings`,
  `use-appearance`, `use-native-behavior`, `use-mobile` (the shadcn sidebar imports it); types
  `session.ts`, `inventory.ts`, the `AppSettings` half of `settings.ts`; `lib/utils.ts`, `lib/fonts.ts`,
  `lib/themes.ts`; the 15 test files that cover the live path and the two IPC fixtures.
- `app/src/components/ui/` is Shadcn-managed. Do not delete anything in it even if unused.

## Delete

### Tauri crate (`app/src-tauri`)

- All 38 commands under `src/actions/agent/` and their registrations in `src/lib.rs`:
  `fetch_providers`, `ping_agent`, `check_auth`, `setup_verify_token`, `setup_github_connect`,
  `setup_complete`, `list_services`, `inspect_service`, `list_service_tasks`, `scale_service`,
  `restart_service`, `rollback_service`, `update_service_env`, `update_service_image`,
  `inspect_task`, `list_task_stats`, `list_stacks`, `remove_stack`, `list_nodes`, `list_node_tasks`,
  `update_node_availability`, `update_node_role`, `list_networks`, `list_volumes`, `list_secrets`,
  `create_secret`, `remove_secret`, `list_configs`, `create_config`, `remove_config`,
  `inspect_config`, `list_cluster_metrics`, `list_node_metrics`, `list_images`, `remove_image`,
  `prune_images`, `get_agent_settings`, `update_agent_settings`.
- Any type or module only those commands used.
- Dependencies that become unused: `reqwest`, `futures-util`, `base64`. Keep `tokio-tungstenite`
  (ADR 0006 uses it). Run `cargo build` to confirm nothing else references them.
- Rename the scaffold: `Cargo.toml` name `kontainer-desktop`, description "Kontainer desktop",
  authors from `git config user.name`; `tauri.conf.json` `productName` "Kontainer". Keep the
  identifier `dev.kontainer.app` (the vault service name matches it). Replace the hard-coded
  "v1.0.0" strings in the splash and login screens with the version from `@tauri-apps/api/app`
  `getVersion()`.

### React (`app/src`)

- Routes: `routes/exec/`, `routes/logs/`, `routes/services/`, `routes/stacks/`, `routes/nodes/`,
  `routes/networks/`, `routes/volumes/`, `routes/images/`, `routes/secrets/`, `routes/configs/`,
  and `routes/_authenticated/index.tsx` (the HomePage that never renders). Regenerate
  `routeTree.gen.ts` by running the dev server or `vite build`.
- Components: `add-cluster-dialog`, `code-editor`, `column-headers`, `command-palette`,
  `dashboard-header`, `dashboard/*`, `env-editor`, `exec/*`, `image-editor`, `log-viewer`, `logs/*`,
  `node-actions`, `node-drain-dialog`, `refresh-button`, `service-detail`, `service-tasks`,
  `services-list`, `setup-wizard`, `sidebar-nav`, `stacks-list`, `status-bar`, `table-shell`,
  `table-toolbar`, `task-detail-sheet`, `terminal-panel`, `terminal-popover`,
  `terminal-session-view`. Spec 03 rebuilds the shell pieces from the design.
- Contexts: `terminal-context*`, `command-palette-context*`.
- Hooks: the 17 legacy queries (`use-services`, `use-stacks`, `use-nodes`, `use-networks`,
  `use-volumes`, `use-secrets`, `use-configs`, `use-config-detail`, `use-service-detail`,
  `use-service-tasks`, `use-task-detail`, `use-task-stats`, `use-node-tasks`, `use-images`,
  `use-cluster-metrics`, `use-agent-settings`, `use-agent-status`), the 15 legacy mutations, the four
  WebSocket hooks (`use-log-stream`, `use-service-logs`, `use-task-logs`, `use-exec-stream`),
  `use-terminal`, `use-command-palette`.
- Types: `services`, `service-detail`, `service-tasks`, `stacks`, `nodes`, `node-tasks`, `networks`,
  `volumes`, `secrets`, `configs`, `images`, `metrics`, `task-detail`, `task-stats`, `tasks`,
  `auth.ts`, `connection.ts`; the `AgentSettings` type in `settings.ts`.
- `lib/task-utils.ts`, `lib/query-client.ts`, `lib/format-utils.ts` if nothing live imports it,
  `assets/react.svg`.
- Auth context compatibility fields: `user`, `token`, `expiresAt`, `agentUrl`, `profiles`,
  `activeProfileId`. Change the `/login` `beforeLoad` guard to check `context.auth.session`.
- The route whitelist in `_authenticated.tsx` that hides every route except `/settings` and
  `/tasks`, and its "Connected" card. Render `<Outlet />` unconditionally. Update the
  `authenticated-shell.spec.tsx` case that asserted the whitelist.
- Tests: the 27 legacy spec files under `app/tests/unit/` (command-palette, setup-wizard,
  images-page, secrets-configs, use-terminal, nodes-list, networks-list, volumes-list, log-viewer,
  env-editor, status-bar, node-actions, image-editor, use-exec-stream, stack-detail, services-list,
  service-detail, logs-page, list-stacks, use-agent-status, service-tasks, scale-service,
  inspect-task, task-logs, rollback, restart, remove-stack) plus `dashboard-overview.spec.tsx`.
  Rename `use-tasks.spec.tsx` to `use-container-inventory.spec.tsx`.
- `package.json`: remove `next-themes`. Font packages are handled in spec 03.

### Repo root

- `dev/Procfile` is replaced in spec 02; leave it.
- Root `package.json` scripts `dev:controller` uses `cargo loco start`; spec 02 fixes it.

## Rules

- Delete whole files; do not leave empty modules or `// removed` comments.
- After each batch run `pnpm types` and `cargo check` in `app/src-tauri` and fix imports.
- Tests are not type-checked today (`tsconfig.json` includes only `src`). Add
  `tests/**/*.ts` and `tests/**/*.tsx` to the `include` list and fix whatever breaks, so the
  remaining tests compile against the real types. If a live test only passes because of a
  compatibility field, rewrite the test, never restore the field.

## Tests

- Existing live tests keep passing. `session-ipc.spec.ts` checks the fixtures against the command
  list in `lib.rs`; update the fixture command list when commands are removed.
- Add one test: `app/tests/unit/dead-code.spec.ts` asserting that no file under `app/src` imports a
  path containing `/actions/agent/`, `use-log-stream`, `command-palette`, or `setup-wizard`. Cheap
  guard against resurrection by autocomplete.

## Done when

- `git grep -n "tauri-app"` returns nothing.
- `pnpm --filter app test`, `pnpm types`, `pnpm lint`, `pnpm fmt` exit 0.
- `cargo fmt`, `cargo test` in `app/src-tauri` exit 0, and `cargo tree` there shows no `reqwest`.
- The app launches, logs in through the existing GitHub-disabled path is not required; the splash,
  login screen, settings, and container list still render (run `pnpm dev:app` once and screenshot
  the login screen with the `run` skill if available, otherwise rely on tests).
- `PROGRESS.md` row 01 is `done` with the commit.
