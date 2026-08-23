# CLAUDE.md

This file contains repo-specific rules that should change implementation behavior. Put framework reference material in skills, not here.

## Project Overview

Kontainer has two parts:

- `server/` - a Rust workspace containing a Loco controller and a Tokio/Tonic node agent
- `app/` - a Tauri desktop app using Rust, React, Shadcn UI, and Tailwind CSS

The primary target today is Docker Swarm. Compose and Kubernetes are planned.

## Skills Activation

This project has domain-specific skills available. You MUST activate the relevant skill whenever you work in that domain; don't wait until you're stuck.

- `frontend-ui` - Activates for React UI work in `app/`. Use when building or restyling screens, composing Shadcn UI components, choosing Tailwind classes, adjusting layout, spacing, dark mode, or responsive behavior.
- `vitest-tauri-testing` - Activates for frontend tests that mock Tauri IPC, events, windows, or other native-layer integrations.
- `tanstack-query-best-practices` - Activates when building data-driven React components that fetch, cache, or mutate server state with TanStack Query.
- `tanstack-router-best-practices` - Activates when adding or modifying routes, loaders, search params, or navigation logic with TanStack Router.
- `react-useeffect` - Activates when writing or reviewing useEffect, deriving state from props, syncing with external systems, or deciding whether an Effect is the right tool.

## Hard Rules

- Follow existing patterns in sibling files before introducing a new approach.
- Reuse existing components, services, and utilities before creating new ones.
- Do not create new top-level directories without approval.
- Do not change dependencies (`go.mod`, `package.json`, `Cargo.toml`) without approval.
- Group code by domain, not by technical layer.
- Use descriptive names for variables, methods, types, and files.

## UI Rules

- For ANY React work, you MUST load `react-useeffect`, `tanstack-query-best-practices`, and `tanstack-router-best-practices` skills before writing code. No exceptions.
- For UI work, load the `frontend-ui` skill before writing code.
- Use Shadcn UI components whenever an equivalent exists.
- If a needed Shadcn component is missing, install it with the Shadcn CLI, then use it.
- Never manually edit `app/src/components/ui/`; that directory is Shadcn-managed.
- Follow existing Tailwind patterns, including dark mode behavior where the surrounding UI already supports it.

## Code Organization

Use the action-per-file pattern for discrete operations.

- Rust actions and Tauri commands should follow the same single-responsibility pattern and use explicit dependency/state injection.

## Server Rules

- The controller exposes `/api/v1`; it is not a proxy to the Docker socket.
- `kontainer-controller` owns the public API, PostgreSQL, authorization, durable operations, and controller-to-agent coordination.
- `kontainer-node-agent` owns node-local Docker access and never connects to PostgreSQL.
- Agents send metrics and check-ins to the controller; only the controller persists central state.
- Keep API responses flat and limited to what the frontend needs.
- Use Loco/Axum middleware and request extractors at the HTTP edge, then pass explicit dependencies into domain actions.
- Use SeaORM entities, typed query builders, relationships, and builder migrations for PostgreSQL. Avoid raw SQL for ordinary persistence.
- PostgreSQL is the only supported runtime database. Loco currently compiles an unused SQLite driver transitively; do not configure or document SQLite as supported.
- Private RPC uses the `kontainer.agent.v1` Tonic contract and mandatory mTLS.
- Ordinary progress uses polling of durable operations. Logs and exec use binary WebSocket. Do not add SSE without a measured need.
- Keep public DTOs, domain types, protobuf messages, persistence models, and desktop IPC types distinct at trust/compatibility boundaries.

## Desktop App Rules

- React talks to Rust via Tauri `invoke`; a UI-neutral Rust client talks to the controller.
- Register every new Tauri command in `app/src-tauri/src/lib.rs`.
- Keep Rust and TypeScript types in sync for any data crossing the Tauri boundary.
- For serialized Rust structs sent to TypeScript, use `#[serde(rename_all = "camelCase")]` unless a sibling pattern clearly differs.
- Session state is stored locally in `session.json` via `tauri-plugin-store`.
- Always use TanStack Query to call Tauri commands from React — wrap `invoke` calls in `useQuery`/`useMutation` hooks instead of calling them directly in components or effects.

## Architecture Notes

- The controller and node agent are separate binaries; do not recreate leader/follower modes in one executable.
- Keep Docker operations behind Kontainer-owned traits such as `ContainerEngine`; do not leak Bollard wire types into domain actions or public responses.
- Supervise long-running reconcilers and RPC/HTTP servers under explicit cancellation and graceful shutdown.
- See `docs/v2/approved-greenfield-reset.md` for authoritative decisions. Older v1/Goravel documents are historical research.

## Testing And Verification

- Every code change must be covered by tests or by updating existing tests.
- Run the smallest relevant test set first, then broader verification when the change touches shared infrastructure.
- Frontend tests use Vitest.
- Do not delete tests without approval.

Run the relevant checks for the code you changed:

- Frontend: `pnpm --filter app test`, `pnpm types`, `pnpm lint`, `pnpm fmt`
- Server workspace: `cargo fmt --manifest-path server/Cargo.toml --all -- --check`, `cargo check --manifest-path server/Cargo.toml --workspace --all-targets`, `cargo clippy --manifest-path server/Cargo.toml --workspace --all-targets -- -D warnings`, `cargo test --manifest-path server/Cargo.toml --workspace`
- Tauri/Rust: `cargo fmt`, `cargo test`

Keep this file short. If guidance becomes tutorial-like, move it into a skill.
