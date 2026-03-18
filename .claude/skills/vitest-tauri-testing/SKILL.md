---
name: vitest-tauri-testing
description: Use when writing or editing Vitest tests for the Tauri frontend, especially when mocking IPC commands, events, or windows
---

# Vitest + Tauri Testing

**Version:** Tauri v2 with `@tauri-apps/api/mocks`

## Overview

Frontend tests use Vitest with Tauri's mock module to simulate the native layer. The test setup at `app/tests/setup.ts` already mocks `@tauri-apps/api/core`, `plugin-deep-link`, and `plugin-opener`.

## Test Structure

- Tests live in `app/tests/feature/` and `app/tests/unit/`
- Run tests: `pnpm --filter app test` or `pnpm --filter app test tests/specific.spec.ts`
- All Frontend/React tests must use Vitest — no other test framework

## WebCrypto Polyfill

jsdom lacks WebCrypto. Add this to any test file that needs it:

```typescript
import { beforeAll } from "vitest";
import { randomFillSync } from "crypto";

beforeAll(() => {
  Object.defineProperty(window, "crypto", {
    value: {
      // @ts-ignore
      getRandomValues: (buffer) => randomFillSync(buffer),
    },
  });
});
```

## Mocking Tauri IPC Commands

Use `mockIPC` from `@tauri-apps/api/mocks` to intercept `invoke()` calls:

```typescript
import { mockIPC } from "@tauri-apps/api/mocks";
import { invoke } from "@tauri-apps/api/core";

test("invoke command", async () => {
  mockIPC((cmd, args) => {
    if (cmd === "add") {
      return (args.a as number) + (args.b as number);
    }
  });

  expect(await invoke("add", { a: 1, b: 2 })).toBe(3);
});
```

For multiple commands, use `vi.mocked(invoke).mockImplementation`:

```typescript
vi.mocked(invoke).mockImplementation(async (cmd) => {
  if (cmd === "load_session") return { token: "jwt", agentUrl: "http://localhost" };
  if (cmd === "list_services") return [{ id: "svc1", name: "web" }];
});
```

## Mocking Events

Enable event mocking via `shouldMockEvents: true`:

```typescript
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { emit, listen } from "@tauri-apps/api/event";

test("mocked event", () => {
  mockIPC(() => {}, { shouldMockEvents: true });

  const eventHandler = vi.fn();
  listen("test-event", eventHandler);

  emit("test-event", { foo: "bar" });
  expect(eventHandler).toHaveBeenCalledWith({
    event: "test-event",
    payload: { foo: "bar" },
  });
});
```

## Mocking Windows

Use `mockWindows` to simulate multi-window setups:

```typescript
import { mockWindows } from "@tauri-apps/api/mocks";

test("windows", async () => {
  mockWindows("main", "second", "third");

  const { getCurrent, getAll } = await import("@tauri-apps/api/webviewWindow");

  expect(getCurrent()).toHaveProperty("label", "main");
  expect(getAll().map((w) => w.label)).toEqual(["main", "second", "third"]);
});
```

## TanStack Router in Tests

Components using `Route.useSearch()` or `useNavigate()` need a full router context:

```typescript
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";

const history = createMemoryHistory({ initialEntries: ["/target-route"] });
const router = createRouter({ routeTree, history });

render(<RouterProvider router={router} />);
```

## Critical Rules

- Always `clearMocks()` after each test to reset mock state
- Always `await act(async () => {})` after render to let `useEffect` mount effects resolve
- Never delete test files without approval
