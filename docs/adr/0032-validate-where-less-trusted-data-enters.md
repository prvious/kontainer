# 0032. Validate where less-trusted data enters a more-trusted component

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/00-goal.md (rule 13)
- Decision record: owner decision D1 (option B) in the PR 135 review

## Context

Rule 13 of `docs/build/00-goal.md` makes "validation at every HTTP and IPC boundary" a security
floor. The desktop read it as both directions. Every value Rust returned to the WebView went
through a hand-written TypeScript parser (`containerListFromIpc`, `nodeListFromIpc`, …) that
checked again what the native client had already checked: exact key sets, the id regexes, the
timestamp format, and the size caps.

That re-check added no safety:

- Rust is the privileged side. It already validates everything that enters it: controller answers
  in the native client (`deny_unknown_fields`, ids, timestamps, caps) and WebView arguments in the
  Tauri commands.
- The WebView is the less-trusted side, because it renders content. Re-checking Rust's own typed
  output there protects nothing. The "secret canary" exact-key check fired only after the field was
  already in WebView memory.
- The parsers came to about 735 lines that mirrored the Rust rules. Each contract change touched
  three layers, and one odd row rejected a whole list, which blanked the page.

## Decision

- **Rule 13's IPC floor means validating data where it enters the more-trusted side.** Two places
  qualify:
  - WebView → Rust: Tauri command arguments are validated in Rust.
  - Controller → native client: responses are validated by the Rust client before any view is built.
- **Rust's output to the WebView is typed, not re-validated.**
  - `authenticatedInvoke<T>`, `invokeSession<T>` and `invokeLogin<T>` return `invoke<T>(…)` directly.
  - Log and exec Channels are typed: `Channel<LogStreamEvent>`, `Channel<ExecStreamEvent>`.
- **What TypeScript still checks:**
  - The operation `progress` and event `detail` documents, which Rust passes through as opaque JSON.
    `nodePullsFromProgress` and `progressCounts` keep their guards.
  - Error values, which are closed over known codes: `inventoryErrorCode`, `sessionErrorCode` and
    `loginFailureFromIpc`.
- **Drift is caught at compile time.**
  - The Rust tests assert that each view serializes to the shared fixture in `app/tests/fixtures/`.
  - The TypeScript tests type the same fixtures against the views with `ipcFixture<View>(fixture)`
    (`app/tests/ipc-fixture.ts`).
  - A renamed, missing or retyped field therefore fails `pnpm types`, whichever side changed.

## Consequences

- One layer changes per field. A list with one odd row renders instead of failing whole.
- A Rust/TypeScript mismatch now shows up at compile time, not as a runtime `invalid_response`.
  This holds only while the fixtures stay shared. Each Rust view test must keep pinning its fixture.
- JSON imports widen string literals, so the compile-time check compares string unions as `string`.
  A new or renamed enum value is caught by the Rust fixture test, not by `pnpm types`.
- 21 validator-only test cases and one credential-canary assertion were deleted with the owner's
  approval.
- HTTP validation is unchanged: the controller validates every request, and the native client
  validates every response.
