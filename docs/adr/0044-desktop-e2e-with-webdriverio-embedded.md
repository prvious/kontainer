# 0044. Drive the desktop app end to end with WebdriverIO's embedded WebDriver

- Status: accepted
- Date: 2026-10-07
- Spec: none

## Context

The desktop app is covered by Vitest component tests and Rust unit tests, but nothing launches the
built app. Tauri's own `tauri-driver` has no macOS support, because Apple ships no WebDriver for
`WKWebView`. CrabNebula's macOS driver needs a paid subscription and an expiring `CN_API_KEY`.
WebdriverIO's Tauri service now has an embedded provider: `tauri-plugin-wdio-webdriver` runs a W3C
WebDriver server inside the app on macOS, Windows and Linux. It is free, MIT-licensed, and needs no
external driver.

## Decision

- **Dependencies.**
  - Rust: `tauri-plugin-wdio-webdriver` 1.4, optional, behind the new `e2e` Cargo feature.
  - npm dev dependencies, all WebdriverIO v9, which is what `@wdio/tauri-service` 1.4 is built on:
    `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`,
    `@wdio/globals`, `@wdio/tauri-service` and `webdriverio`.
  - `pnpm-workspace.yaml` denies the build scripts pnpm asked about. `edgedriver` and `geckodriver`
    only download browser drivers, which the embedded provider doesn't use. `esbuild` and `msw`
    never ran their scripts in this repo before, and still don't.
- **Never in a shipped build.** The plugin hands control of the app to anything that can reach its
  local port. So it is registered only under `#[cfg(feature = "e2e")]`, and `tauri build` without
  `--features e2e` doesn't compile it. No capability grants it anything: its default permission set
  is empty.
- **Its own identity.** The `e2e` build uses the identifier `dev.kontainer.e2e`
  (`tauri.e2e.conf.json`), which gives it a separate `session.json`, `settings.json` and window
  state. It also uses the vault service `dev.kontainer.e2e`. A test run never reads, rewrites or
  signs out the developer's real saved connections and keychain credentials.
- **How it runs.**
  - `pnpm --filter app e2e:build` builds a debug binary with the frontend bundled.
  - `pnpm --filter app e2e` runs `app/tests/e2e/*.e2e.ts` with Mocha against that binary.
  - The specs have their own `tsconfig.json`, so WebdriverIO's globals stay out of the app's types.
  - Vitest only collects `*.spec.ts(x)`, so it never picks up the e2e specs.
- **Scope today.** The Connect smoke test and seven connection scenarios exercise the built webview,
  real Tauri IPC and Rust HTTP client. A loopback HTTP fixture supplies bootstrap and rejected-login
  replies and rejects unknown endpoints. Successful authentication, process-restart restoration,
  inventory, mutations and real Swarm streaming remain the next journeys to add.
  All spec files share one worker/session so automatic service recovery between workers cannot hide
  an application crash.

## Consequences

- macOS, Windows and Linux use the same suite and provider.
- CI uses `.github/workflows/desktop.yml`: one matrix produces Linux, Windows and macOS jobs. Each
  runs frontend units, Rust units, native-vault qualification and the same E2E suite. Linux runs
  the suite under Xvfb inside a D-Bus session with an unlocked Secret Service keyring; macOS unlocks
  its login keychain. Triggers include the native client, its stream dependencies and workspace
  manifests. Local macOS execution is verified; Linux and Windows runtime qualification comes from
  their native CI jobs.
- The `e2e` feature must stay out of release tooling. A release built with it would ship a WebDriver
  server.
- WebdriverIO v10 exists, but `@wdio/tauri-service` 1.4 targets v9. Upgrade them together.
