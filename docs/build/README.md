# docs/build

The execution plan for the Kontainer desktop v1 build. One autonomous agent works through the
numbered specs in order; the owner reviews `PROGRESS.md` and `docs/adr/`.

| File | Purpose |
|------|---------|
| `00-goal.md` | Standing instructions, ground rules, spec order, definition of done, the `/goal` condition to paste |
| `01-cleanup.md` … `18-integrations.md` | One spec per feature, in build order |
| `PROGRESS.md` | Ledger: one row per spec, updated before every commit |
| `design-gaps.md` | Screens and dialogs the design does not draw yet, with interim behaviour |

Design source: `docs/design.pen`, read through the Pencil MCP with the full path. Node ids in the
specs refer to that file.

Decisions already made: `docs/v2/approved-greenfield-reset.md` and `docs/adr/`.
