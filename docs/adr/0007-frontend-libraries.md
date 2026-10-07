# 0007. Frontend libraries: Recharts, TanStack Table, no schema library, fixed fonts

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/01-cleanup.md, docs/build/03-shell-and-foundations.md

## Context

The app already depends on React 19, TanStack Router and Query, shadcn on base-ui primitives,
Tailwind v4, CodeMirror, ghostty-web, sonner, lucide, and Recharts (unused). The design defines six
chart primitives and seven list pages with sorting, filtering, and row selection. Six small forms need
validation. The design fixes three typefaces; the app ships a picker with seven fonts.

## Decision

- Charts: Recharts renders every chart primitive (bar sparkline, ring, track, stacked track, state
  cells). The owner adjusts the design to match Recharts output; the agent does not hand-roll SVG.
- Tables: `@tanstack/react-table` is added and drives every list page through the shadcn data-table
  pattern (headless, column defs, sorting, filtering, row selection).
- Forms: no zod, no react-hook-form. Plain validator functions per form; the strict hand-written IPC
  parsers in `app/src/types/` stay the boundary contract. Revisit when a form passes five fields.
- Fonts: Space Grotesk (sans), JetBrains Mono (mono), Chivo (brand mark) only. The font picker and
  the six unused `@fontsource-variable` packages are removed.
- Removed as unused: `next-themes`; Rust crates `futures-util` and `base64` in the Tauri crate.
  `tokio-tungstenite` stays because ADR 0006 uses it.

## Consequences

- One dependency added, seven removed. Bundle and install shrink.
- The design file is the source of truth for chart look; where Recharts cannot match it, the design
  moves, not the library.
