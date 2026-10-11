# Desktop and server testing

Desktop testing uses one workflow with Linux, Windows and macOS matrix jobs.
Each runs frontend and Rust units, qualifies the native credential store, then
drives the built app with the embedded WebDriver. Prioritize E2E coverage of
user journeys and retain deterministic tests for contracts, security and
failure paths.

## CI layout

| Workflow | Jobs and coverage |
| --- | --- |
| `desktop.yml` · Desktop | Linux, Windows, macOS: units, native vault, built-app E2E. Frontend type, lint and format checks run on Linux. |
| `server.yml` · Server | Required PostgreSQL workspace tests and stream evidence; independent release builds and executable smoke on Linux amd64 and arm64. |
| `docker.yml` · Docker | Real isolated daemons pinned to 27.5.1 and 29.7.2; manually selected API fixtures as a backup. |

There is no separate desktop E2E workflow. Desktop triggers include the native
client's streaming dependencies and workspace manifests. Linux runs the whole
WebDriver command under Xvfb inside an unlocked Secret Service D-Bus session.
Windows uses WebView2, and macOS unlocks its login keychain.

Build the E2E binary after ordinary Cargo tests so later integration-test
builds cannot overwrite it. Docker qualifications require one executed test
per exact filter and the engine/inventory evidence markers.

CI disables dev/test debug symbols, preserves workspace build artifacts in
Cargo caches, installs Linux packages without recommended extras and keeps
cache writes on main. Superseded PR runs cancel; main runs finish so they can
save caches. Native protobuf tests use the release artifacts already built.
Clippy covers workspace compilation, so the separate server `cargo check` is
removed. Native release jobs also cover binary construction, replacing the
duplicate debug binary builds. Stream evidence comes from the workspace test
run, preserving frame, TLS-graph and lifecycle assertions.

Before these changes, [desktop run 37579981305](https://github.com/prvious/kontainer/actions/runs/37579981305)
took 48 seconds on macOS, 180 on Linux and 219 on Windows. Linux package
installation took 61 seconds. Cargo cache restoration took 46 seconds on
Linux and 62 on Windows; the Windows archive was 837 MiB, with about 46 seconds
spent extracting it. [Server run 37557228154](https://github.com/prvious/kontainer/actions/runs/37557228154)
took 852 seconds, including 188 seconds building debug binaries after tests.
Compare the first cold and warm runs of the new workflows before claiming a
measured speedup. Their changed cache environment intentionally starts fresh.

## WebDriver and E2E coverage

The installed `@wdio/tauri-service` and `tauri-plugin-wdio-webdriver` 1.4.0
provide executors for all three operating systems. The explicit embedded
provider needs no external WebDriver. [WebdriverIO platform support](https://webdriver.io/docs/desktop-testing/tauri/platform-support)

The Connect smoke and seven scenarios run against the built webview,
real Tauri commands and Rust HTTP client. A loopback controller fixture covers
owner/no-owner bootstrap, invalid owner fields, remote HTTP rejection,
rejected-password handling, malformed replies and connection recovery.
Unexpected fixture endpoints fail. No successful login is issued, so these
tests leave no saved fixture session. The E2E app has its own identifier and
vault service (ADR 0044).

The specs share one worker and session, preventing the service's automatic
between-worker crash recovery from hiding a failed application process.

Next E2E journeys, in order:

1. Successful sign-in, authenticated navigation, sign-out and login redirects.
2. Sign-in, quit the desktop process, relaunch, restore and sign out; a webview
   refresh or WebDriver `reloadSession()` does not test process-start restoration.
3. Settings save and appearance persistence across a process restart.
4. Inventory filtering, sorting and navigation with search state surviving a
   reload.
5. A confirmed mutation followed through durable pending and terminal states.
6. Real-Swarm logs and exec, including input, resize and cleanup on reload or
   window closure.

Use controller HTTP fixtures for cross-platform desktop integration and the
real controller, PostgreSQL, agents and DinD Swarm for Docker behavior. A fake
controller cannot qualify Docker scheduling, overlay networks or exec.

Successful-login fixtures must also remove their saved connection during
cleanup: signing out deliberately preserves it. Restoration tests should
finish their process-restart assertions before cleaning the isolated profile.

## Counselors review

Astra, Fable, Sol and Opus each completed at least three passes. They agreed on the three workflows, native platform qualification,
successful-login and process-restart priorities, and assertion-level unit
retirement. Keep normal-feature desktop checks and ARM64 qualification.
Enabling E2E everywhere does not eliminate Tauri's configuration differences.
Workspace caching still needs timing evidence.

Fable and Opus initially proposed removing the duplicate manual fake job;
it remains as the requested isolated backup, adding no duplicate push/PR run.
Their broader Connect-test retirement proposals narrowed after comparing
blur timing, mode transitions, IPC arguments and diagnostics. The final
retirement table below preserves those distinct assertions.

## Unit test retirement

Remove duplicated scenarios individually. Keep each test until its replacement
covers the same assertions; removal still requires the owner's approval.

| Candidate | Replacement or reason | Keep |
| --- | --- | --- |
| `token-expiry.spec.tsx`, both cases | Existing `session-persistence.spec.tsx` already tests restored views and invalidated profiles. These tests do not exercise expiry in Rust. | Rust expiry, rotation and failed-restore tests. |
| `redirect-after-login.spec.tsx`, both cases | Its local route tree duplicates routing logic rather than exercising the application's login route. Replace with a successful-login E2E using the real router. | Redirect validation and safe navigation behavior. |
| Overlapping cases in `connect-card.spec.tsx` | Retire only assertions matched by Connect E2E once all three OS jobs pass. The current suite replaces rejected-password behavior more fully than the bootstrap, validation or retry cases. | Accessibility, same-card mode switching, blur timing, mismatch validation, full refusal diagnostics, lockout details, saved connections and probe concurrency. |
| Basic shell/sidebar/resource render cases | Replace after authenticated navigation and inventory E2E exists. | Empty, stale and partial data; failed mutations; cancellation; stream races. |
| Source-text assertions in desktop `session/tests/logs.rs` | Replace the checks for page-load/window-destruction hook strings once real reload/close E2E observes backend cleanup. | The registry cancellation behavior tested in the same function. |

Retain the inexpensive `use-auth.spec.ts` provider assertion, IPC fixture checks,
secret redaction, vault/store commit ordering, tamper rejection, concurrent
refresh/restore, uncertain cleanup, idempotency,
TLS and bounded binary-stream tests. These are cheap, deterministic checks of
cases ordinary E2E cannot reliably produce. All 310 frontend tests passed in
the local baseline; that run provides no evidence for calling them flaky.

## Docker fixture and backup

The development fixture already runs three privileged `docker:dind`
containers through `dev/docker-compose.yml`: one manager and two workers.
On 2026-10-07, all ran Docker 29.8.0, all nodes were Ready/Active, and all eight
seed services had their requested replicas. All ten engine dev-cluster tests
and the controller's scale/restart/rollback convergence test passed against
this live cluster. Existing volumes and seeded services were preserved.

[Vercel Labs emulate](https://github.com/vercel-labs/emulate) does not advertise
a Docker service. [go-dockerclient's testing package](https://pkg.go.dev/github.com/fsouza/go-dockerclient@v1.13.2/testing)
offers a programmable fake Docker API server, but introducing a Go helper
would duplicate this repository's existing Rust socket fixtures. Reuse those
fixtures instead of adding a dependency or implementing a Swarm emulator.

If real-Docker infrastructure is unavailable, manually dispatch `Docker` with
`backend=fake`. The `API fixture` job runs the engine's library tests, including
scripted Docker HTTP negotiation, inventory and failure cases. Push and PR
runs always use the pinned real daemons; a failed real qualification remains
a failure. This fallback preserves API-contract feedback and provides no
real-engine or Swarm compatibility evidence.

These tests also run in Server; the manual job isolates the requested backup
from its PostgreSQL service and broader workspace checks.

## Verification

Local verification passed: eight macOS desktop E2E tests; 310 frontend units;
131 desktop Rust units plus the native vault test; the PostgreSQL-required
server workspace suite; ten real-Swarm engine tests and controller operation
convergence; release protobuf tests; frontend type/lint/format checks; Rust
format/check/Clippy, including real-engine test sources; actionlint and
shellcheck.

Linux and Windows native desktop execution and new CI timings remain to be
qualified by GitHub runs. Check-name changes may require updating repository
protection rules if enabled; this repository's current GitHub plan returned
403 for both protection and ruleset inspection.
