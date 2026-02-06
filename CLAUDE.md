# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Devoper is a lightweight desktop app for managing containerized infrastructure — a self-hosted alternative to Portainer for small teams and solo developers.

## Architecture

Two main components:

1. **Agent (Go)** — Stateless Go binary deployed alongside infrastructure. Exposes REST + WebSocket API that talks to the Docker socket. Handles OAuth authentication (GitHub, Google) with a Laravel Socialite-inspired flow and issues short-lived JWTs.

2. **Desktop App (Tauri + React)** — Cross-platform desktop app built with Tauri (Rust) and React. Connects to one or more agents, handles OAuth login via system browser, stores credentials in OS keychain.

## Tech Stack

- Desktop: Tauri (Rust) + React
- Agent: Go (stdlib)
- Auth: OAuth 2.0 + JWT
- Transport: HTTPS + WebSocket
- App Storage: OS Keychain + SQLite
- Agent Storage: Stateless (in-memory only)

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

## Index Route

```tsx
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
    component: () => <div>Home</div>,
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

## Layout Routes

routes/dashboard/
├── \_layout.tsx
├── index.tsx
└── settings.tsx

```tsx
import { Outlet, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/dashboard")({
    component: () => <Outlet />,
});
```

## Navigation

```tsx
import { Link } from '@tanstack/react-router'

<Link to="/about">About</Link>
<Link to="/posts/$postId" params={{ postId: '123' }} />
```

## Notes

- \_\_root.tsx is required
- index.tsx = default route
- $param.tsx = URL param
- \_layout.tsx = shared layout
- Route tree generation gives full type safety

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

## Development Commands

### Core Development

```bash
# Start Vite dev server only (port 1420)
pnpm dev

# Start Tauri development mode with hot reload (RECOMMENDED)
pnpm tauri dev

# Preview production build
pnpm preview
```

### Building

```bash
# Build frontend (TypeScript compilation + Vite build)
pnpm build

# Build Tauri application for production
pnpm tauri build
```

### Testing

```bash
# Run Playwright tests
pnpm test

# Run tests in UI mode
pnpm test:ui

# Run specific test file
pnpm test test tests/example.spec.ts
```

### Rust Development

```bash
# Check Rust code (from src-tauri/)
cd src-tauri && cargo check

# Format Rust code
cd src-tauri && cargo fmt

# Run Rust tests
cd src-tauri && cargo test
```

## Architecture

### Frontend-Backend Communication

The app uses Tauri's IPC (Inter-Process Communication) to call Rust functions from React:

```typescript
// Frontend (React/TypeScript)
import { invoke } from "@tauri-apps/api/core";

// Calls the create_bucket Tauri command
await invoke("create_bucket", {
    input: { bucketName, region, acl, connectionId },
});
```

```rust
// Backend (Rust)
#[tauri::command]
async fn create_bucket(
    state: tauri::State<'_, AppState>,
    input: CreateBucketInput,
) -> Result<CreateBucketResponse, ErrorResponse> {
    // Implementation
}
```

All Tauri commands must be registered in `src-tauri/src/lib.rs` in the `invoke_handler!` macro.

### Type Synchronization (CRITICAL)

When modifying data structures passed between Rust and TypeScript:

1. **Update Rust first:** Modify struct in `src-tauri/src/models/*.rs`
2. **Update TypeScript:** Match the interface in `src/types/*.ts`
3. **Verify serialization:** Ensure serde attributes match camelCase in TypeScript
    ```rust
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]  // This converts snake_case to camelCase
    pub struct MyStruct {
        pub bucket_name: String,  // Becomes "bucketName" in JSON
    }
    ```

### Error Handling

**Structured Error System:**

- Backend errors defined in `src-tauri/src/errors/mod.rs` using `thiserror`
- All errors convert to `ErrorResponse` with `error_type` and `message` fields
- Frontend receives consistent error format from all Tauri commands

**Error Types:**

- Validation errors (empty fields, invalid bucket names)
- S3 operation errors (AWS SDK errors)
- Internal errors (connection not found, serialization failures)

Frontend should handle errors from `invoke()` calls:

```typescript
try {
    await invoke("create_bucket", { input });
} catch (error) {
    // error is already a structured ErrorResponse
    console.error(error);
}
```

## Code Style

### TypeScript/React

- **File Naming:** kebab-case for components and utilities (e.g., `connection-manager.tsx`)
- **Component Structure:** Functional components, React 19 patterns (no React import needed for JSX)
- **Imports:** Use `@/` alias for absolute imports (configured in vite.config.ts and tsconfig.json)
- **Styling:** Tailwind CSS 4 utility classes, shadcn/ui components
- **Types:** Strict TypeScript (`strict: true` in tsconfig), define interfaces in `src/types/`
- **Formatting:** This project uses prettier, Use `pnpm format` before committing
- **Quality Checks:** Always run `pnpm run lint` and `pnpm run format` after finishing all tasks in a session

### Rust

- **Edition:** Rust 2021
- **Formatting:** Use `cargo fmt` before committing
- **Error Handling:** Use `thiserror` for custom error types, propagate with `?`
- **Async:** All S3 operations are async, using Tokio runtime
- **Serialization:** Use `#[serde(rename_all = "camelCase")]` for structs sent to frontend
- **Modules:** Organize by feature (models, s3, errors), use `pub` visibility intentionally

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
