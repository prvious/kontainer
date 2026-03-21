# CLAUDE.md

This file contains repo-specific rules that should change implementation behavior. Put framework reference material in skills, not here.

## Project Overview

Kontainer has two parts:

- `server/` - a Go agent built with Goravel
- `app/` - a Tauri desktop app using Rust, React, Shadcn UI, and Tailwind CSS

The primary target today is Docker Swarm. Compose and Kubernetes are planned.

## Skills Activation

This project has domain-specific skills available. You MUST activate the relevant skill whenever you work in that domain; don't wait until you're stuck.

- `frontend-ui` - Activates for React UI work in `app/`. Use when building or restyling screens, composing Shadcn UI components, choosing Tailwind classes, adjusting layout, spacing, dark mode, or responsive behavior.
- `goravel-tooling` - Activates when writing or updating artisan commands, Goravel tests, or framework-integrated logging.
- `goravel-data` - Activates when working with ORM models, relationships, queries, migrations, factories, or seeders.
- `goravel-foundations` - Activates when working with service providers, the service container, facades, configuration, or application bootstrap and lifecycle wiring.
- `goravel-async` - Activates when working with queues, jobs, events, listeners, task scheduling, or caching.
- `goravel-security` - Activates when implementing authentication, authorization, JWT guards, policies, hashing, or encryption.
- `goravel-http` - Activates when building routes, controllers, middleware, request validation, request parsing, or HTTP responses.
- `goravel-integrations` - Activates when working with gRPC, outbound HTTP clients, filesystem/storage, mail, or localization.
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

- Go example: `services/scale_service.go` defines `ScaleServiceAction` with `Execute`
- Rust/Tauri commands should follow the same single-responsibility pattern and use state injection

## Agent Rules

- The agent exposes its own API; it is not a proxy to the Docker socket.
- Keep API responses flat and limited to what the frontend needs.
- Auth routes are public; `/api/*` routes require JWT.
- Real-time flows such as logs and auth bridging use WebSocket.

### Goravel

- Prefer Goravel facades and framework primitives over ad hoc native Go implementations.
- Prefer ORM models and relationships over raw queries; use eager loading to avoid N+1 issues.
- Use dedicated request structs for validation instead of inline controller validation.
- Use environment variables only in config files; read config through Goravel config APIs in application code.
- When relevant, load the matching `goravel-*` skill instead of duplicating framework reference here.

## Desktop App Rules

- React talks to Rust via Tauri `invoke`; Rust talks to the Go agent.
- Register every new Tauri command in `app/src-tauri/src/lib.rs`.
- Keep Rust and TypeScript types in sync for any data crossing the Tauri boundary.
- For serialized Rust structs sent to TypeScript, use `#[serde(rename_all = "camelCase")]` unless a sibling pattern clearly differs.
- Session state is stored locally in `session.json` via `tauri-plugin-store`.
- Always use TanStack Query to call Tauri commands from React — wrap `invoke` calls in `useQuery`/`useMutation` hooks instead of calling them directly in components or effects.

## Architecture Notes

- The agent runs in `leader` or `follower` mode based on `AGENT_ROLE`.
- Leaders serve the API and scheduled work; followers expose internal gRPC and health endpoints.
- Leader discovery and follower registration live under `server/app/services/cluster/`.
- Read the nearby cluster files before changing discovery, registration, or role-specific bootstrap behavior.

## Testing And Verification

- Every code change must be covered by tests or by updating existing tests.
- Run the smallest relevant test set first, then broader verification when the change touches shared infrastructure.
- Frontend tests use Vitest.
- Do not delete tests without approval.

Run the relevant checks for the code you changed:

- Frontend: `pnpm --filter app test`, `pnpm types`, `pnpm lint`, `pnpm fmt`
- Go agent: `gofmt`, `go vet -C server ./...`, `go test -C server ./...`
- Tauri/Rust: `cargo fmt`, `cargo test`

Keep this file short. If guidance becomes tutorial-like, move it into a skill.
