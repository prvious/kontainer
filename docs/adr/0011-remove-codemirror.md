# 0011. Remove CodeMirror with the v1 editors

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/01-cleanup.md

## Context

Spec 01 deletes `code-editor`, `env-editor`, and `image-editor`, the only importers of
`@uiw/react-codemirror`, `@codemirror/lang-json`, `@codemirror/lang-yaml`, and
`@codemirror/theme-one-dark`. No later spec edits code in the desktop: deploy stack and service
editing are deferred by ADR 0010. The same spec drops the Tauri crate's direct `reqwest`; it still
arrives transitively through `kontainer-native-client`, which is the intended controller client.

## Decision

- Remove the four CodeMirror packages from `app/package.json`.
- "No `reqwest` in `cargo tree`" is read as "no direct `reqwest` dependency of the Tauri crate";
  `cargo tree --depth 1` is the check.

## Consequences

- Smaller install and bundle. A future compose or env editor re-adds an editor through its own
  spec and ADR.
