# Build goal: Kontainer desktop v1

You are the implementing agent. This file is your standing instruction set. The numbered specs in
this folder are your task definitions. `PROGRESS.md` is your ledger. `docs/adr/` holds your decisions.
Nobody will answer questions while you work; every spec states its defaults.

## Mission

Turn the Kontainer desktop app and server into a working single-user Docker Swarm operator tool that
matches `docs/design.pen` feature by feature, by implementing `01-cleanup.md` through
`18-integrations.md` in order. When the goal is met the owner can create their account, sign in,
browse containers, images, volumes, networks, stacks, services, and nodes of the development cluster,
run the mutations the specs list, stream service logs, open a shell in a container, and change
settings, all from the desktop app.

## Ground rules

1. **Never stop to ask.** When a spec is silent, choose the simplest option consistent with
   `CLAUDE.md`, `docs/v2/approved-greenfield-reset.md`, and the ADRs, then record the choice as an
   ADR. The ponytail ladder applies: does it need to exist, is it already in the codebase, does the
   standard library or an installed dependency cover it, can it be one line, only then write code.
2. **Read the design through the Pencil MCP** with the full path
   `/Users/clovismuneza/Apps/kontainer/docs/design.pen`. Use `Get` with visitors to read text and
   structure, `Export` to PNG when you need to look. Never `Read` or `Grep` the `.pen` file. Node ids
   in the specs are stable; screen names are `Kontainer — <Page>` and states are
   `State NN.Sn · <Page> — <Condition>`. Each state has a caption frame next to it whose text is the
   acceptance criterion for that state.
3. **Design fidelity.** Layout, copy, and states come from the mockups. Mock numbers are placeholders;
   real data replaces them. Deferred features render exactly as `docs/adr/0010` and
   `design-gaps.md` say. Never invent a screen for a gap.
4. **Branch and commits.** Work only on `build/desktop-v1`. Commit whenever a coherent unit passes its
   checks, several commits per spec are fine. Message format: `<area>: <what> (spec NN)`. Never
   force-push, never merge to `main`, never open a pull request.
5. **Ledger.** Update `PROGRESS.md` before every commit: the spec's status, a one-line note, and the
   commit once done. Append a line to its log at the end of every work session.
6. **ADRs.** Write one (`docs/adr/NNNN-slug.md`, template in `docs/adr/README.md`, add it to the
   index) whenever you deviate from a spec or the reset doc, add or remove a dependency, change a
   contract (HTTP, protobuf, Tauri IPC, database), or choose between two routes a reviewer would ask
   about. Numbering continues from the highest existing.
7. **Blocked?** Write a `blocked` row in `PROGRESS.md` with what you tried and the exact error, then
   continue with the next spec that does not depend on it. Retry blocked specs once after finishing
   the rest.
8. **Checks.** Run the smallest relevant set after each change and the full set before marking a spec
   done. A spec is not done while any check fails.
   - Frontend: `pnpm --filter app test`, `pnpm types`, `pnpm lint`,
     `pnpm --filter app fmt:check` (`pnpm fmt` is the fixer; it rewrites files and can't fail)
   - Server: `cargo fmt --manifest-path server/Cargo.toml --all -- --check`,
     `cargo check --manifest-path server/Cargo.toml --workspace --all-targets`,
     `cargo clippy --manifest-path server/Cargo.toml --workspace --all-targets -- -D warnings`,
     `cargo test --manifest-path server/Cargo.toml --workspace` with PostgreSQL running from
     `server/compose.postgres.yaml` and `KONTAINER_REQUIRE_POSTGRES_TESTS=1` plus
     `KONTAINER_TEST_DATABASE_URL` set as `README.md` documents
   - Tauri: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` in
     `app/src-tauri`
9. **Tests.** Every change ships with tests or updated tests. Frontend tests mock Tauri IPC the way
   `app/tests/setup.ts` already does (load the `vitest-tauri-testing` skill). Server tests follow
   `server/src/test_support.rs` for PostgreSQL. Real Docker tests use the development cluster from
   spec 02 behind the existing `real-engine-tests` feature. Never delete a test that covers live code.
10. **Skills.** Before any React work load `frontend-ui`, `react-useeffect`,
    `tanstack-query-best-practices`, and `tanstack-router-best-practices`. Use Shadcn components
    whenever one exists; install missing ones with the Shadcn CLI; never edit
    `app/src/components/ui/` by hand.
11. **Dependencies.** Pre-approved additions: `@tanstack/react-table`, `@tanstack/react-virtual`,
    `@fontsource-variable/chivo` (or `@fontsource/chivo`), and `oxfmt` as a dev dependency if
    `pnpm fmt` needs it. Pre-approved removals are listed in spec 01 and spec 03.
    Anything else needs an ADR and must pass the ponytail ladder.
12. **Architecture invariants** (from `CLAUDE.md` and the reset doc): React never talks to the
    controller, only to Tauri commands; every Tauri command is registered in `app/src-tauri/src/lib.rs`
    and mirrors its TypeScript type; the controller owns the public API and PostgreSQL; the node agent
    never touches PostgreSQL; Docker stays behind `ContainerEngine`-style traits; Bollard types never
    cross into domain actions or DTOs; public DTOs, domain types, protobuf messages, persistence
    models, and IPC types stay distinct; mutations are durable operations polled by the client;
    logs and exec are binary WebSocket; private RPC stays mTLS.
13. **Security floors that are never simplified:** validation at every HTTP and IPC boundary
    (where data enters the more-trusted side; Rust's typed output to the WebView is not
    re-validated, see ADR 0032), Argon2id for passwords, generic authentication errors, login throttling, no secrets or
    credentials in logs or in `session.json`, mTLS untouched, destructive actions that can lose
    data require typed confirmation; others use a plain confirm (ADR 0036).
14. **Real cluster.** The docker-in-docker Swarm from spec 02 is yours to use and reset. Never run
    Docker mutations against anything except the `kontainer-cluster-*` containers.
15. **Scope discipline.** Build what the spec says and nothing beyond it. Deferred items in
    `docs/adr/0010` stay deferred even when they look easy.

## Order and gates

Specs run in numeric order. Each spec ends with a "Done when" list; that list is the gate. Specs 05
and 06 are shared infrastructure that specs 07 to 16 depend on, so their tests must be thorough.

## Definition of done for the whole goal

- Every row in `PROGRESS.md` is `done`, or `blocked` with a written reason after one retry.
- All checks in rule 8 exit 0, shown in the transcript.
- `git status` is clean and every commit is on `build/desktop-v1`.
- The mission paragraph is true against the development cluster.

## The /goal condition to paste

Paste this as one line after `/goal` (it is under the 4,000 character limit):

```
Read docs/build/00-goal.md and implement docs/build/01-cleanup.md through docs/build/18-integrations.md in order, following every rule in 00-goal.md. The goal is met when: docs/build/PROGRESS.md marks every spec done (or blocked with a written reason after one retry); the transcript shows `pnpm --filter app test`, `pnpm types`, `pnpm lint`, `pnpm --filter app fmt:check`, `cargo fmt --manifest-path server/Cargo.toml --all -- --check`, `cargo clippy --manifest-path server/Cargo.toml --workspace --all-targets -- -D warnings`, `cargo test --manifest-path server/Cargo.toml --workspace` (with KONTAINER_REQUIRE_POSTGRES_TESTS=1), and `cargo fmt --check` and `cargo test` in app/src-tauri all exiting 0 in the final turn; every commit is on branch build/desktop-v1; and `git status` is clean. Report progress against the spec list every turn.
```
