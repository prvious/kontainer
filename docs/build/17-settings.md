# 17. Settings

## Goal

The Settings page with staged edits, validation, the agent URL change confirmation, and the typed
controller setting from spec 05. Replaces the current tabbed settings screen.

Design: Settings `8wGs5` (light `BVIeL`); states `zDHFf` (08.S5 unsaved changes and validation),
`e2KKmA` (08.S6 change agent URL confirm, saved toast); captions `Z4Rb8t`, `N5AaZ`. Gaps G14
(Integrations entry), G22 (auth provider row), G26 (theme row).

## Server

- `GET /api/v1/status` already returns version; add `logLevel` (the effective tracing filter) and
  `startedAt` so the sidebar Agent Info box and this page can show version, uptime, and log level.
- Settings routes exist from spec 05 (`metricsRetentionDays`).

## Desktop

- Route `/settings`. Delete the old `settings/*` tab components; keep `AppSettings` in the Tauri
  store minus the `font` field (spec 03) and add `theme: "system" | "light" | "dark"` if the
  appearance scheme is not already stored there (merge the two stores into `AppSettings` if they
  are separate, tolerating old files).
- Header: `SETTINGS`, `// application & agent configuration`.
- Sections and rows (each row: label, mono description, control on the right):
  - APPLICATION: Theme select system / light / dark (G26); Dashboard Refresh Interval select
    2s / 5s / 10s / 30s / 60s; Default Terminal Shell select auto / /bin/bash / /bin/sh / /bin/zsh /
    custom (text field appears); Terminal Font Size number stepper 10–24; Terminal Scrollback
    Buffer select 1,000 / 5,000 / 10,000 / 50,000.
  - AGENT with the version badge from `/status`: Agent URL text input; Auth Provider read-only
    `Password`; Log Level read-only with a tooltip "Set with RUST_LOG on the controller"; Metrics
    Retention select 1 / 3 / 7 / 14 / 30 days (PATCH `/api/v1/settings`); Signed in as
    `display name · email` read-only; Uptime read-only.
  - INTEGRATIONS (G14): one row "External services" with an OPEN button to
    `/settings/integrations`.
- Staged edits (08.S5): controls edit local form state, nothing auto-saves. Changed rows get the
  `--primary-faint` tint and a `changed from <old>` note; invalid fields validate on blur with the
  reason under the control (URL must be http or https with a host; custom shell must be an
  absolute path; font size in range). A sticky bottom bar shows `N pending changes · M errors`
  with SAVE (disabled while any error) and DISCARD (reverts everything). Leaving the page with
  pending changes asks for confirmation through the router's `useBlocker`.
- Saving (08.S6): application settings write through `save_app_settings`; the retention writes
  through `update_settings`; both then show one success toast `Settings saved`. If the Agent URL
  changed, a non-destructive ConfirmDialog (neutral tile, primary button) spells out the
  consequences: signing out, signing in on the new agent, open terminals and log streams closing.
  Confirm signs out and navigates to `/login` with the new origin prefilled; Cancel keeps the old
  URL and the other pending changes.

## Tests

- Staged bar counts; validation cases; DISCARD reverts; blocker on navigation; URL change dialog
  paths; retention PATCH called with the selected value; theme change applies immediately after
  save.

## Done when

- Changing the refresh interval speeds up polling visibly; changing the retention persists across
  a controller restart; the URL change flow lands on `/login` with the new origin; all checks pass;
  `PROGRESS.md` updated.
