# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Kontainer is a desktop app for managing containerized infrastructure (Docker Swarm, with Compose and Kubernetes planned). It consists of two components:

- **Agent** — A Go (Chi) HTTP/WebSocket API deployed alongside the infrastructure, talks to the Docker socket
- **Desktop App** — A Tauri (Rust + React + ShadcnUI + Tailwindcss) app that connects to one or more agents

## UI Requirements (MANDATORY)

**Shadcn UI is REQUIRED for all UI work. This is non-negotiable.**

- You MUST always use Shadcn UI components before even considering creating custom components. If Shadcn has a component for it, use it. No exceptions.
- If a needed Shadcn component is not yet installed, install it first (`pnpm dlx shadcn@latest add <component>`), then use it.
- Do NOT create custom UI components when a Shadcn equivalent exists. Exhaust all Shadcn options first.
- When working on any UI task, you MUST activate the `frontend-design` and `shadcn-ui` skills before writing any code.
- The `src/components/ui/` directory is a NO-GO zone — never manually edit, extend, or fix files in it. These are Shadcn-managed.

## Conventions

### Action Pattern (Both Go and Rust)

Every discrete operation is a single-responsibility action. One action per file. Actions receive their dependencies via constructor injection.

**Go (Agent):**

- File: `services/scale_service.go`
- Struct: `ScaleServiceAction` with an `Execute` method
- Injected via `main.go`

**Rust (Tauri):**

- Follow the same pattern for Tauri commands — one command per file, dependencies injected via Tauri's state management

### Project Structure

Group related code by domain, not by type:

```
# ✅ Do this
services/
  list_services.go
  scale_service.go

# ❌ Not this
actions/
  list_services.go
  scale_service.go
handlers/
  services_handler.go
```

### API Design

- The agent exposes its own clean API — it is NOT a proxy to the Docker socket
- Response shapes should be flat and minimal — only what the frontend needs
- Auth routes are public, all `/api/*` routes require JWT
- Real-time features (logs, auth flow) use WebSocket with polling fallback

### Auth

- Laravel Socialite-inspired OAuth flow (redirect → callback → JWT)
- The agent handles the full OAuth dance; the desktop app just receives a deeplink with the JWT from the agent once auth flow is done
- JWTs are short-lived, stored in `session.json` via `tauri-plugin-store`

### General

- Keep dependencies minimal — prefer stdlib where reasonable
- Go agent is mostly stateless — the only exception is an embedded SQLite database (`modernc.org/sqlite`, pure Go) used by the leader to store time-series node metrics with automatic retention-based pruning. All other state is in-memory only.
- Desktop app stores session data locally via `tauri-plugin-store` (`session.json`)

## Repository Structure

Monorepo managed with pnpm workspaces:

- `/app` — Desktop app (Tauri + React). Frontend in `app/src/`, Rust backend in `app/src-tauri/`.
- `/agent` — Go agent binary deployed alongside infrastructure. Exposes REST + WebSocket API to Docker socket.

## Development Commands

All commands run from the repo root:

```bash
pnpm dev              # Run both app and agent in parallel
pnpm dev:app          # Tauri desktop app only (hot reload)
pnpm dev:agent        # Go agent only (go run)
pnpm build            # Build both
pnpm build:app        # Build desktop app
pnpm build:agent      # Build Go agent → agent/bin/agent
pnpm lint             # Lint frontend
pnpm fmt           # Format frontend
```

Run commands in a specific workspace:

```bash
pnpm --filter app <script>     # Run a script in /app
```

## Tech Stack

- Desktop: Tauri (Rust) + React + ShadcnUI + Tailwindcss
- Agent: Go (stdlib, Chi)
- Auth: OAuth 2.0 + JWT
- Transport: HTTPS + WebSocket
- App Storage: tauri-plugin-store (session.json)
- Agent Storage: In-memory + embedded SQLite for metrics history (leader only)

## Supported Orchestrators

- Docker Swarm (primary)
- Docker Compose (planned)
- Kubernetes (planned)

=== .ai/shadcn rules ===

# Shadcn UI Components

This application uses Shadcn UI components for consistent, accessible, and customizable UI elements.

## Component Usage

**Always use Shadcn UI components when building UI features.**

## Installation Workflow

If a required Shadcn component is not yet installed in the project:

1. Install the component using the Shadcn CLI
2. Continue with the implementation work using the newly installed component

## Examples

### Installing Components

- `pnpm dlx shadcn@latest add button`
- `pnpm dlx shadcn@latest add card`
- `pnpm dlx shadcn@latest add dialog`

## Important Notes

- Components are installed to the `src/components/ui/` directory
- Shadcn components are fully customizable and use Tailwind CSS
- Always check if a component exists before installing to avoid duplicates
- Components are designed to be accessible and follow best practices
- Never attempy to manually edit,extend, or fix anything in the `src/components/ui`. the directory contains components installed from shadcnui. this is a NO GO zone.
- The components rely on base-ui implementation instead of radix-ui. so, prefer using `render={() => [element we want to render as the child]}` instead of using the `asChild` prop on the element you'd like to replace

=== tailwindcss/core rules ===

## Tailwind CSS

- Use Tailwind CSS classes to style HTML; check and use existing Tailwind conventions within the project before writing your own.
- Offer to extract repeated patterns into components that match the project's conventions (i.e. JSX/TSX).
- Think through class placement, order, priority, and defaults. Remove redundant classes, add classes to parent or child carefully to limit repetition, and group elements logically.

### Spacing

- When listing items, use gap utilities for spacing; don't use margins.

<code-snippet name="Valid Flex Gap Spacing Example" lang="html">
    <div class="flex gap-8">
        <div>Superior</div>
        <div>Michigan</div>
        <div>Erie</div>
    </div>
</code-snippet>

### Dark Mode

- If existing pages and components support dark mode, new pages and components must support dark mode in a similar way, typically using `dark:`.

=== tailwindcss/v4 rules ===

## Tailwind CSS 4

- Always use Tailwind CSS v4; do not use the deprecated utilities.
- `corePlugins` is not supported in Tailwind v4.
- In Tailwind v4, configuration is CSS-first using the `@theme` directive — no separate `tailwind.config.js` file is needed.

<code-snippet name="Extending Theme in CSS" lang="css">
@theme {
  --color-brand: oklch(0.72 0.11 178);
}
</code-snippet>

- In Tailwind v4, you import Tailwind using a regular CSS `@import` statement, not using the `@tailwind` directives used in v3:

<code-snippet name="Tailwind v4 Import Tailwind Diff" lang="diff">
   - @tailwind base;
   - @tailwind components;
   - @tailwind utilities;
   + @import "tailwindcss";
</code-snippet>

### Replaced Utilities

- Tailwind v4 removed deprecated utilities. Do not use the deprecated option; use the replacement.
- Opacity values are still numeric.

| Deprecated | Replacement |
|------------+--------------|
| bg-opacity-_ | bg-black/_ |
| text-opacity-_ | text-black/_ |
| border-opacity-_ | border-black/_ |
| divide-opacity-_ | divide-black/_ |
| ring-opacity-_ | ring-black/_ |
| placeholder-opacity-_ | placeholder-black/_ |
| flex-shrink-_ | shrink-_ |
| flex-grow-_ | grow-_ |
| overflow-ellipsis | text-ellipsis |
| decoration-slice | box-decoration-slice |
| decoration-clone | box-decoration-clone |

=== tanstack-router rules ===

# TanStack Router: File-Based Routing

## File Structure = Routes

src/
└── routes/
    ├── \_\_root.tsx
    ├── index.tsx
    ├── about.tsx
    └── posts/
        ├── index.tsx
        └── $postId.tsx

| File              | URL            |
| ----------------- | -------------- |
| index.tsx         | /              |
| about.tsx         | /about         |
| posts/index.tsx   | /posts         |
| posts/$postId.tsx | /posts/:postId |

## Root Route

```tsx
import { Outlet, createRootRoute } from "@tanstack/react-router";

export const Route = createRootRoute({
    component: () => (
        <>
            {" "}
            <Outlet />{" "}
        </>
    ),
});
```

## Page Routes

```tsx
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/about")({
    component: () => <div>About</div>,
});
```

## Dynamic Routes

routes/posts/$postId.tsx

```tsx
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/posts/$postId")({
    component: () => {
        const { postId } = Route.useParams();
        return <div>{postId}</div>;
    },
});
```

=== vitest rules ===

## Vitest

### Test rules

- Every change must be programmatically tested. Write a new test or update an existing test, then run the affected tests to make sure they pass.
- Run the minimum number of tests needed to ensure code quality and speed.
- If you need to verify a feature is working, write or update a unit / feature test.
- All Frontend/React tests must be written using Vitest.
- You must not remove any tests or test files from the tests directory without approval. These are not temporary or helper files - these are core to the application.
- Tests should test all of the happy paths, failure paths, and weird paths.
- Vitest Tests live in the `tests/feature` and `tests/unit` directories.
- When writing your frontend tests, having a “fake” Tauri environment to simulate windows or intercept IPC calls is common, so-called mocking. The @tauri-apps/api/mocks module provides some helpful tools to make this easier for you
- Remember to clear mocks after each test run to undo mock state changes between runs! See clearMocks()(https://tauri.app/reference/javascript/api/namespacemocks/#functions) docs for more info.

<code-snippet name="Mocking Commands for invoke" lang="typescript>
import { beforeAll, expect, test } from "vitest";
import { randomFillSync } from "crypto";

import { mockIPC } from "@tauri-apps/api/mocks";
import { invoke } from "@tauri-apps/api/core";

// jsdom doesn't come with a WebCrypto implementation
beforeAll(() => {
Object.defineProperty(window, 'crypto', {
value: {
// @ts-ignore
getRandomValues: (buffer) => {
return randomFillSync(buffer);
},
},
});
});

test("invoke simple", async () => {
mockIPC((cmd, args) => {
// simulated rust command called "add" that just adds two numbers
if(cmd === "add") {
return (args.a as number) + (args.b as number);
}
});
});
</code-snippet>

<code-snippet name="Mocking Events" lang="typescript>
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { emit, listen } from '@tauri-apps/api/event';
import { afterEach, expect, test, vi } from 'vitest';

test('mocked event', () => {
mockIPC(() => {}, { shouldMockEvents: true }); // enable event mocking

const eventHandler = vi.fn();
listen('test-event', eventHandler);

emit('test-event', { foo: 'bar' });
expect(eventHandler).toHaveBeenCalledWith({
event: 'test-event',
payload: { foo: 'bar' },
});
});
</code-snippet>

<code-snippet name="Mocking Windows" lang="typescript>
import { beforeAll, expect, test } from 'vitest';
import { randomFillSync } from 'crypto';

import { mockWindows } from '@tauri-apps/api/mocks';

// jsdom doesn't come with a WebCrypto implementation
beforeAll(() => {
Object.defineProperty(window, 'crypto', {
value: {
// @ts-ignore
getRandomValues: (buffer) => {
return randomFillSync(buffer);
},
},
});
});

test('invoke', async () => {
mockWindows('main', 'second', 'third');

const { getCurrent, getAll } = await import('@tauri-apps/api/webviewWindow');

expect(getCurrent()).toHaveProperty('label', 'main');
expect(getAll().map((w) => w.label)).toEqual(['main', 'second', 'third']);
});
</code-snippet>

### Testing

```bash
# Run Vitest tests
pnpm --filter app test

# Run specific test file
pnpm --filter app test tests/example.spec.ts
```

### Rust (from app/src-tauri/)

```bash
cargo check
cargo fmt
cargo test
```

### Go Agent (from agent/)

```bash
go run .
go build -o bin/agent .
go test ./...
```

## Current Architecture

### Multi-Node Cluster (Leader/Follower)

The agent binary runs in two modes controlled by `KONTAINER_ROLE`:

- **Leader** — Full REST + WebSocket API. Handles auth, serves all `/api/*` routes, discovers and healthchecks follower agents. One per cluster.
- **Follower** — Lightweight. Only exposes `/internal/health` and `/internal/info` behind a shared secret (`X-Kontainer-Secret` header). Deployed as a global Swarm service across worker nodes.

**Discovery:** Leader finds followers by querying Docker Swarm tasks for the follower service name (`KONTAINER_FOLLOWER_SERVICE`), pings each task IP on `/internal/health` every 10s, and maintains an in-memory agent registry.

**Dev mode:** Set `KONTAINER_DEV_FOLLOWERS` to a comma-separated list of static addresses instead of Swarm-based discovery.

Key files:

- `agent/cluster/discovery.go` — Swarm-based follower discovery (production)
- `agent/cluster/dev_discovery.go` — Static follower list (development)
- `agent/cluster/registry.go` — Thread-safe in-memory agent registry
- `agent/cluster/middleware.go` — Internal secret auth middleware
- `agent/server/server.go` — Leader routes
- `agent/server/follower.go` — Follower routes

### Frontend-Backend Communication

React → Tauri IPC (`invoke`) → Rust HTTP client → Go agent API.

```typescript
// Frontend calls Tauri command
const services = await invoke("list_services", { url: agentUrl, token });
```

```rust
// Rust command makes HTTP request to agent
#[tauri::command]
async fn list_services(url: String, token: String) -> Result<ListServicesResponse, String> {
    // GET {url}/api/services with Bearer token
}
```

All Tauri commands registered in `src-tauri/src/lib.rs` `invoke_handler!` macro.

### Type Synchronization (CRITICAL)

When modifying data structures passed between Rust and TypeScript:

1. **Update Rust first:** Modify struct in the relevant action file under `src-tauri/src/actions/`
2. **Update TypeScript:** Match the interface in `src/types/*.ts`
3. **Verify serialization:** Ensure serde attributes match camelCase in TypeScript
    ```rust
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct MyStruct {
        pub bucket_name: String,  // Becomes "bucketName" in JSON
    }
    ```

### Data Flow

- **React hooks** (`src/hooks/`) use TanStack Query with 5s refetch intervals
- **Mutation hooks** call Tauri commands and invalidate relevant query keys
- **Auth context** (`src/contexts/auth-context.tsx`) provides `token` and `agentUrl` to all hooks
- **WebSocket features** (logs, auth bridge) connect directly from the Rust layer or React to the agent

### Error Handling

All Tauri commands return `Result<T, String>`. The agent returns structured JSON errors with HTTP status codes. Frontend handles errors from `invoke()`:

```typescript
try {
    await invoke("scale_service", { url, token, serviceId, replicas });
} catch (error) {
    console.error(error); // string error message
}
```

## Code Style

### TypeScript/React

- **File Naming:** kebab-case for components and utilities (e.g., `connection-manager.tsx`)
- **Component Structure:** Functional components, React 19 patterns (no React import needed for JSX)
- **Imports:** Use `@/` alias for absolute imports (configured in vite.config.ts and tsconfig.json)
- **Styling:** Tailwind CSS 4 utility classes, shadcn/ui components
- **Types:** Strict TypeScript (`strict: true` in tsconfig), define interfaces in `src/types/`
- **Formatting:** This project uses oxfmt, Use `pnpm fmt` before committing
- **Quality Checks:** Always run `pnpm types`, `pnpm run lint` and `pnpm fmt` after finishing all tasks in a session

### Rust

- **Edition:** Rust 2021
- **Formatting:** Use `cargo fmt` before committing
- **Error Handling:** Use `thiserror` for custom error types, propagate with `?`
- **Async:** All Tauri commands are async, using Tokio runtime
- **Serialization:** Use `#[serde(rename_all = "camelCase")]` for structs sent to frontend
- **Modules:** Organize by feature under `actions/{domain}/`, use `pub` visibility intentionally

### Path Aliases

Both frontend and backend use path aliases:

- TypeScript: `@/components` → `src/components`
- Configured in: `vite.config.ts`, `tsconfig.json`, `components.json`

## Configuration Files

- `vite.config.ts`: Vite config with React plugin, Tailwind, path aliases
- `tsconfig.json`: TypeScript strict mode, ES2020 target, path aliases
- `src-tauri/tauri.conf.json`: Tauri app config (window size, permissions, etc.)
- `src-tauri/Cargo.toml`: Rust dependencies
- `components.json`: shadcn/ui configuration
