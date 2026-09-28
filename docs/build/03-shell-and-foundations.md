# 03. Shell and foundations

## Goal

Build the application chrome and every reusable component from the design's Foundations board so
the page specs only compose. After this spec the app shows the real sidebar on every route, dark
and light themes from the design tokens, and a component kit whose names match the design.

Design: board `lcwus` (00 · Foundations) with sections APP CHROME `uYLCG`, CONTROLS `lA9Nb`,
TABLE PARTS `gwEtM`, OVERLAY `c3Dx8`, LOGO `Cxivk`, SIDEBAR `QOfW1`, STATES & FEEDBACK `EAPpo`.
Chart rules: `EjLoT` (F1 · Chart dosage) and `B64BE` (F2 · Chart library). Brand: `x5Qmv`.
Reference screens: Overview `WCBzg` (dark) and `gQVTN` (light). Decisions: ADR 0007.

## Theme tokens

- Read `GetVariables()` from the design and copy every `--*` colour for `mode=dark` and
  `mode=light` into `app/src/app.css` as CSS custom properties, dark under `.dark`, light under
  `:root`. For the `status` axis use the `current` values only (success `#2DD4A7` dark,
  `#12805F` light; warning `#F59E0B` / `#B45309`; destructive `#FF4757` / `#D6303C` and their
  `-soft` and `-faint` variants).
- Map them in `@theme inline` so Tailwind utilities like `bg-card`, `text-muted-foreground`,
  `text-success`, `bg-primary-soft` exist. Radii `--radius-sm` 4, `-md` 6, `-lg` 8, `-xl` 12.
  Text scale `--text-3xs` 10 through `--text-4xl` 36 as Tailwind font sizes (`text-3xs`, `text-2xs`).
- Delete the v1 palette (`accent-green/cyan/purple/orange`, `bg-elevated`, `.grain`) and
  `lib/themes.ts` once nothing references them.
- Fonts: `--font-sans` Space Grotesk, `--font-mono` JetBrains Mono, `--font-brand` Chivo. Add
  `@fontsource-variable/chivo` (or `@fontsource/chivo` weights 700 and 900 if no variable build
  exists) and remove `dm-sans`, `figtree`, `inter`, `nunito-sans`, `outfit`, `plus-jakarta-sans`.
  Delete `lib/fonts.ts`, the appearance font picker, and the `font` field in `AppSettings` and its
  Rust twin (spec 17 owns the settings page; here only remove the picker and keep the settings
  file backward-tolerant by ignoring the unknown field).
- Appearance context: `scheme` system/light/dark already exists. Make it toggle the `.dark` class
  from the resolved scheme instead of always adding it, keep the native window theme sync. The
  Toaster theme follows the same value instead of hard-coded dark.

## App chrome

- **Accent Line** (`09e1f`): the 2px primary line across the top of every window. One component.
- **Titlebar** (`iHwhS`): transparent native title bar with the macOS traffic lights on the left
  (existing drag-region handling stays), then Logo Mark Small (`WgGUD`) + "KONTAINER" in the brand
  font; centre: a pill with a status dot, the cluster label, and a chevron that opens the cluster
  switcher (spec 04); right: search and bell icons are not rendered (design-gaps G13).
- **Sidebar** (`YJkf8`): use the installed shadcn `sidebar` primitives styled to the design.
  Sections with `// INFRASTRUCTURE`, `// ORCHESTRATION`, `// SYSTEM` labels; items in this order
  with lucide icons: Overview `layout-dashboard`, Containers `box` (count badge), Images `layers`,
  Volumes `hard-drive`, Networks `network`, Stacks `file-stack`, Nodes `server` (badge "N DOWN" when
  any node is down), Services `activity`, Exec `terminal`, Logs `scroll-text`, Settings `settings`.
  Active item: background `--sidebar-accent`, foreground `--sidebar-accent-foreground`, bold.
  Badges accept a number or text and hide when zero. Bottom Agent Info box: controller version and
  uptime (from `/api/v1/status`, spec 04 exposes it through a Tauri command; until then render the
  version only). Navigation uses TanStack Router `Link` with `activeProps`; no `useEffect` for
  active state.
- **Status bar** (`KYxT3`): segments left to right: Docker version, `CPU 47%`, `MEM 12.4/24G`,
  `24 TASKS`, `1 ALERT` (destructive tint when > 0); right: `2 TERMINALS`, `3 LOG STREAMS`,
  `WS CONNECTED` / `RECONNECTING` (controller reachability). The branch and Traefik segments from
  the design are not rendered. Build it as a presentational component fed by a small `StatusBarData`
  object; specs 05, 11, 15, 16 fill the fields; until then each segment shows `—`.
- **Layout**: `_authenticated.tsx` renders Accent Line, Titlebar, a row with Sidebar and
  `<Outlet />`, then Status Bar. Main content scrolls, chrome does not. Minimum window size
  1100 × 700 set in `tauri.conf.json`; content cards scroll horizontally when narrower than the
  design (design-gaps G20).
- **Page header** (`page-header.tsx` exists; restyle): title in caps, mono subtitle line, right-side
  actions slot. Every page spec names its title and subtitle text.
- **Splash** (`PwbGj`): logo mark, "KONTAINER", "Infrastructure, Simplified", progress track with
  "Initializing runtime…", version from Tauri, faint grid lines, corner labels. State `gcAoz`
  (02.S1 startup failed): after 15 s without a session-restore answer, or on a restore error that is
  not "no session", the bar turns destructive and an inline error card shows cause, error code,
  request id when present, and RETRY (re-runs restore). OPEN LOGS renders disabled with tooltip
  "Not built yet" (ADR 0010). Never an endless spinner.

## Component kit (`app/src/components/`)

Names match the design so specs can reference them. Compose from shadcn; install what is missing
with the Shadcn CLI (`alert` at least; check `badge`, `tooltip`, `skeleton`, `dialog`, `table`,
`checkbox`, `dropdown-menu`, `sheet`, `popover`, `progress` are present).

| Component | Design | Behaviour |
|---|---|---|
| `Button` variants | `z5Rrz` primary (lime), `BSMVd` secondary, `qXPxh` outline; `jeLSJ` is the same primary and maps to it | states default / hover / `pending` (keeps width, shows spinner, blocks double submit) / disabled / `restricted` (lock icon, Tooltip Rich explains why). Labels are caps mono `text-xs` 700. |
| `StatusBadge` | `Wbday` (dot + text), `Badge` `hcuLC` (text only), `StatusChip` `4nr0v` (`UP` / `PAUSED` / `EXITED`) | tone: success, warning, destructive, neutral. |
| `Tooltip`, `TooltipRich` | `rXjCV`, `vNg0C` | icon-only actions get a label tooltip after 400 ms; rich variant has title + reason. |
| `Toast` | `y1xaD` + error / warning / loading variants | via sonner, bottom-right, 5 s, optional action button; success and failure stack. |
| `Alert` | `yDnxo` + destructive / info / success | persistent condition at the top of the affected region, optional action. |
| `SkeletonStat`, `SkeletonRow` | `tLPEH`, `ZZAe7` | first load only; refetches keep old rows and spin the header refresh icon. Rows fade toward the bottom. |
| `EmptyState` | `SJK65` | tile icon, title, body, primary + secondary actions. |
| `ErrorState` | `X7KWf` | title, body, code + request id line, RETRY and COPY DETAILS (copies code, request id, message). |
| `ConfirmDialog` | `KIuxe` | title, body, optional impact list, optional typed confirmation (confirm button at 40 % opacity and disabled until the text matches exactly), Cancel / Esc close with no side effects, confirm button `pending` while the operation runs. |
| `FieldError` pattern | `JRLoZ` | label + input + helper; invalid: destructive border and helper with `circle-alert`. Validate on blur and on submit. |
| `ChartTooltip` | `V1sxxr` | Recharts custom tooltip: time range line, series rows, delta line. |
| `OperationStatus` | `nBaer` + queued / succeeded / failed | title, status line (`RUNNING · 4 of 5 · 12s`), track, CANCEL when cancellable. Spec 06 wires it. |
| `TableToolbar` | `w9xU5` | search box, "Filters (n)" button, sort dropdown, filter chips row with "Clear filters". |
| `RowActions` | `KPhXT`, `Ulm5I` | icon buttons with tooltips plus a `⋮` dropdown menu. |
| `DataTable` | table parts `gwEtM`, header/row styling from `WCBzg` | see below. |
| Charts | `B64BE` | `SparkBars`, `Ring`, `Track`, `StackedTrack`, `StateCells` as thin Recharts wrappers. One accent series; status colours mean state, never quantity; every chart sits beside its number; gaps render as "no data", never 0. Hover snaps to the nearest bucket and dims the others (state `UWkEX`). |

### DataTable

`components/data-table/` built on `@tanstack/react-table` (add it) and the shadcn `table`:

- column definitions with sorting; global search; filter chips; row selection with a header
  checkbox that goes indeterminate; a bulk bar that replaces the toolbar when any row is selected
  (state `q5WTPa`, 03.S5) with the selected count, actions slot, CLEAR, and Esc to deselect;
- `isFirstLoad` renders `SkeletonRow` × 6; `isRefetching` spins the header icon and keeps rows;
- an `emptyState` slot rendered inside the table body when there are zero rows after filtering,
  naming the active filters with CLEAR FILTERS and SHOW ALL (state `IU3ve`, 03.S6);
- an `errorState` slot;
- row hover lifts the row and reveals row actions (state `VfepO`, 03.S4);
- count badge in the section header ("ALL CONTAINERS 24").

## Tests

- Theme: toggling the scheme adds or removes `.dark`; both palettes define every token (a test
  reads `app.css` and asserts the token list from the design is present in both blocks).
- ConfirmDialog: typed confirmation enables only on exact match; Esc calls no side effect.
- ErrorState: COPY DETAILS writes code, request id, and message to the clipboard mock.
- DataTable: sort, search, filter chips, selection → bulk bar, empty slot with filters named.
- Sidebar: renders the eleven items in order, active item from the router, badges hide at zero.
- Status bar: renders `—` for missing fields and the destructive tint for alerts > 0.

## Done when

- Every existing route renders inside the new chrome; `/tasks` still lists containers inside it.
- Light mode is selectable and matches the light screens' colours.
- The component kit exists with the tests above; `pnpm --filter app test`, `pnpm types`,
  `pnpm lint`, `pnpm fmt` exit 0.
- `package.json` shows `@tanstack/react-table` added, the font packages and `next-themes` gone,
  `recharts` still present.
