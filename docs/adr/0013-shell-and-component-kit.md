# 0013. Shell and component kit decisions

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/03-shell-and-foundations.md

## Context

Spec 03 builds the chrome and the Foundations kit on shadcn (base-ui) and Recharts. Installing the
pre-approved `@tanstack/react-table` resolved to v9, whose API differs from the v8 shape the shadcn
data-table guide uses, and adding shadcn components pulled in two packages the spec did not name.

## Decision

- **TanStack Table v9** (the current major) with explicit features: column filtering + global
  filtering, sorting, row selection. Pages describe columns with a small `DataTableColumn` shape
  (`cell`, optional `sortValue` and `searchValue`); `DataTable` maps it to v9 column definitions.
- **New packages from the shadcn CLI:** `cmdk` (required by the shadcn `command` component that
  design-gaps G10 and G11 prescribe for the log and exec pickers) and `cn` (shadcn's own class
  merge package, which current registry components import). Existing `ui/` files keep
  `@/lib/utils`. Nothing in `ui/` is edited by hand.
- **Kit names** follow the design components without shadowing shadcn names: `ActionButton`
  (Button Primary/Secondary/Outline), `StatusBadge` / `LabelBadge` / `StatusChip`, `LabelTooltip` /
  `TooltipRich`, `InlineAlert`, `notify` + `AppToaster`, `SkeletonStat` / `SkeletonRow`,
  `EmptyState` / `ErrorState`, `ConfirmDialog`, `FormField`, `OperationStatus`, `ChartTooltip`,
  `TableToolbar` / `BulkBar` / `RowActions` / `DataTable`, and the charts `SparkBars`, `Ring`,
  `Track`, `StackedTrack`, `StateCells`. They live in `components/kit`, `components/data-table`,
  `components/charts`, and `components/shell`.
- **Sidebar items whose route does not exist yet** render disabled with the plain tooltip
  "Not built yet"; each page spec turns its item into a link when it adds the route. `/` redirects to
  `/tasks` until the Overview (spec 14).
- **App settings** are one TanStack Query (`["app", "settings"]`) written through one mutation;
  appearance and polling derive from it. `["app", …]` queries survive sign-in changes. The old
  settings provider, which reset `scheme` on every save, is gone.
- **Splash:** the main window is shown as soon as React renders, so the full-size splash and its
  failure state are visible. `vault_unavailable`, `local_state_failed`, `controller_unavailable`,
  and `invalid_response` from restore are startup failures with RETRY; `relogin_required`,
  `cleanup_uncertain`, and `vault_write_failed` continue to the sign-in screen, which has their
  recovery actions. Fifteen seconds without an answer shows `restore_timeout`.

## Consequences

- Page specs compose from the kit and never style tables or dialogs from scratch.
- A future shadcn CLI run keeps adding `cn`-based files; `@/lib/utils` and `cn` coexist.
