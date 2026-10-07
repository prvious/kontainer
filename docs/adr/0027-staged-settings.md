# 0027. Staged Settings, the controller status, and switching agents

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/17-settings.md

## Context

Spec 17 replaces the old tabbed Settings screen with the design's single page, where edits are
staged. Some rows are stored on this device (theme, refresh interval, terminal defaults), one is
stored on the controller (metrics retention), some are read-only facts about the controller
(version, log level, uptime), and one moves the whole session to another agent (Agent URL).
Nothing reported the controller's start time or log level before this spec.

## Decision

- **Contract addition.**
  - `GET /api/v1/status` gains `logLevel` and `startedAt` (RFC 3339, UTC, milliseconds).
    Both are fixed at boot in a `ControllerRuntime` in the shared store.
  - `logLevel` follows Loco's own order: `RUST_LOG`, then the configured `override_filter`,
    then the configured level. Blank values don't count.
  - The native client gains `get_status`, which refuses unknown fields and control characters.
  - Tauri gains the command `get_status` with a `ControllerStatusView`. The nodes IPC fixture
    carries its shape.
- **Sidebar.** The Agent Info box shows the controller version and its uptime, counted on the
  shared one-second clock.
- **Page.**
  - One route, `/settings`, with APPLICATION, AGENT (with the version badge), and INTEGRATIONS
    sections. The old tab components are gone, and `useSettings` keeps only its reads.
  - `AppSettings` keeps `scheme` as the theme; it was already stored there, so the Tauri store
    didn't change.
  - Edits are local form state kept over the stored values:
    - A row that differs gets the `--primary-faint` tint and `Changed from <old>`.
    - Invalid fields show their reason after blur.
    - The sticky bar counts changes and errors, and SAVE stays off while any error remains.
    - DISCARD drops every edit.
    - `useBlocker` asks before leaving with changes.
  - A stored value that the new lists don't offer (such as the old 3 s interval) stays in its
    select, so the page never shows a blank control.
  - Saving writes the application rows through `save_app_settings` and the retention through
    `update_settings`, then shows one `Settings saved` toast. The toast has UNDO, which the
    design shows, and which writes the previous values back.
- **Deviations.**
  - **Log Level is read-only**, as the spec says, even though state 08.S5 shows it editable
    with a RESTART REQUIRED badge. The controller reads its filter at boot from `RUST_LOG` or
    config. The row's tooltip says to set it with `RUST_LOG` on the controller.
  - **Agent URL validation is Connect's rule**: https, or http only for a loopback host. The
    spec allows any http or https URL with a host. Keeping the security floor, and one rule
    for the same field on two screens, wins over the looser wording.
  - **The Integrations OPEN button is restricted** until the hub route arrives with spec 18
    (G14).
- **Switching agents.**
  - A changed Agent URL opens the neutral confirm dialog. It names the open terminals and log
    streams that will close.
  - Confirming saves the other staged rows first, then signs out and opens Connect with the
    new origin filled in. Cancel keeps every edit.
  - The page test found a race. After sign-out, the router still saw the old session until
    the next render, so `/login` bounced to `/` and the shell then opened Connect without the
    origin. The cluster switcher had the same flaw.
  - `logout` now takes the next origin, and the shell's signed-out redirect carries it. Both
    callers use one `useSwitchAgent` hook.
- **Kit.** `ConfirmDialog` takes `mutates` (default true). Choices that stay on this device,
  such as leaving with unsaved edits or switching away from an unreachable controller, pass
  false, so they aren't paused while the controller is unreachable.
- `DEFAULT_TERMINAL` moved from the `useSettings` hook to `types/settings.ts`, because it is
  data the form reads as well as the hook.

## Consequences

- The status response is additive. Older desktops ignore the new fields, because the old
  client didn't read `/status`.
- Changing the log level still means restarting the controller with a new `RUST_LOG`. The
  Settings page says so and doesn't pretend to change it.
