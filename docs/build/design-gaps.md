# Design gaps

Screens, dialogs, and states the build needs that `docs/design.pen` does not draw yet.
Each row says where the gap shows up, what the implementation does until a design exists,
and whether the interim behaviour is acceptable to ship. Add the design, then update the
"Interim" column to "designed" with the new node id.

Node ids refer to `docs/design.pen`. Open the file with the Pencil MCP using its full path.

## Rules for the implementing agent

- Never invent a screen for a "Missing" row. Use the interim behaviour exactly as written.
- Interim dialogs are built from Foundations components only: Confirm Dialog `KIuxe`,
  Field Error `JRLoZ`, Empty State `SJK65`, Error State `X7KWf`, buttons `z5Rrz` / `BSMVd` /
  `qXPxh` / `jeLSJ`, badges `Wbday` / `hcuLC`.
- Every interim dialog follows the Create Volume dialog (state `iPOa2`) for layout: title,
  one column of labelled fields, helper text under invalid fields, Cancel + primary action.

## Gaps

| # | Gap | Referenced from | Interim behaviour | Status |
|---|-----|-----------------|-------------------|--------|
| G1 | Create network dialog | Networks page `isFfK`, header button CREATE NETWORK | Minimal dialog modelled on Create Volume `iPOa2`: name, driver (overlay / bridge), attachable toggle, node picker for bridge, optional subnet (spec 10, ADR 0020). | Built (interim) |
| G2 | Create service dialog | Services page `LMjiS`, header button CREATE SERVICE | Button disabled with a "Not built yet" tooltip ("Changes are paused" while the swarm has no quorum). Feature deferred. | Deferred (button built, spec 11) |
| G3 | Deploy stack flow (compose editor, validation, plan preview) | Stacks page `twXyi`, Overview `WCBzg` DEPLOY button, empty state `Y1Nv0` | Buttons disabled with a "Not built yet" tooltip; the empty state links the Docker stack deploy docs (spec 12). Feature deferred by the reset doc. | Deferred (buttons built) |
| G4 | Service detail (tasks per service, task state, placement, last error) | Services rows; state `Y3UuOQ` hover text mentions "why the last task can't be placed" | No detail page. Task-level reasons surface only in the row tooltip, from `GET /api/v1/services/{id}` (spec 11). | Built (tooltip only) |
| G5 | Container detail / inspect | Containers rows `2rYBA`, Overview table | No detail page. Row menu offers Logs, Exec, and lifecycle actions only. | Missing |
| G6 | Row "⋮" menu contents for containers, services, stacks | `2rYBA`, `LMjiS`, `twXyi` action columns | Containers: Logs, Exec, Restart, Stop / Start, Pause / Unpause, Remove. Services: Scale, Restart, Rollback, Logs. Stacks: Remove. Rendered with the shadcn dropdown menu. | Missing |
| G7 | Remove container confirmation | Container row and bulk actions | Confirm Dialog `KIuxe` without typed confirmation for a single stopped container; typed name when the container is running; bulk remove lists the names and needs typed count. | Missing |
| G8 | Remove volume confirmation | Volumes page `jigga` row action | Adjusted to Docker's rules (spec 09, ADR 0019): Docker refuses to remove a mounted volume even with force, so an attached volume's trash is disabled with a Tooltip Rich listing its containers; an unattached volume opens a Confirm Dialog with its name typed to confirm (the data is not recoverable). | Built |
| G9 | Stack detail (services grouped by stack) | Stack rows `twXyi` | No detail page. The row menu's Show services filters the Services page by `?stack=` (spec 12). | Built |
| G10 | Log stream picker ("New Stream") | Logs page `ZkgXL` header button and "+" tab | shadcn Command dialog listing services with search; choosing one opens its tab, or focuses the tab it already has (spec 15). | Built (interim) |
| G11 | Exec target picker ("+" tab) and custom-shell retry | Exec page `f8D6u`; state `p1V0Po` mentions retry with a custom shell | Command list of running containers grouped by node; custom shell is a text field in the Error State block. Built as a shadcn Command dialog behind `+` and NEW SHELL, and a shell field with RETRY on `shell_not_found` (spec 16). | Built (interim) |
| G12 | Registry credentials for private pulls | State `D9mu7` says registry errors show "the fix (credentials)" | No credentials UI. Pull errors show the registry message verbatim; private registries unsupported this pass. | Missing, deferred |
| G13 | Titlebar search and notifications panels | Titlebar `iHwhS` right side icons | Icons hidden. Command palette deleted. | Missing, deferred |
| G14 | Settings entry point for Integrations | Traefik breadcrumb `lXH8d` says Settings › Integrations, but Settings `8wGs5` has no Integrations section | Add an "INTEGRATIONS" section header at the bottom of Settings with one row linking to the hub `r9tns`. Built as the last Settings section; OPEN goes to the hub (specs 17–18). | Built |
| G15 | Integrations hub GitHub card in the not-connected state | `r9tns` GitHub card shows CONNECTED | Mirror the Discord card pattern: NOT CONNECTED badge, disabled CONNECT with tooltip "Arrives with deployments". Built per spec 18 and ADR 0010 instead: NOT CONNECTED badge with skeleton description and footer (no CONNECT); the card opens the GitHub page (ADR 0028). | Built (interim) |
| G16 | Deployments and Vault navigation placement | Sidebar `YJkf8` has no entries; Deployments screens override the Services item with a rocket icon | No nav entries. Sections deferred entirely. | Missing, deferred |
| G17 | Add-node enrollment flow end to end | State `MjblZ` draws the dialog; controller enrollment isn't built | ADD NODE is disabled with a "Not built yet" tooltip, in the header and the empty state; the dialog isn't built (spec 13). Dev cluster uses static certificates from `dev/`. | Deferred (button built) |
| G18 | Volume Explorer and File Preview backend | `5mQoG`, `mhztj`, states `D5EBq`, `NZNrn` | No route. The volume row action "Browse files" renders disabled with tooltip "Not built yet". | Designed, deferred |
| G19 | "Load more history" control for logs | State `f3Rq5` caption mentions a way to load more history | The no-match Empty State's LOAD PREVIOUS HOUR reopens the stream with `since = now − 1h` and a 5,000-line tail (spec 15). | Built (interim) |
| G20 | Narrow window layout | All screens are 1440 × 900 | Minimum window 1100 × 700; tables scroll horizontally inside their card; sidebar stays fixed. | Missing |
| G21 | Session expired dialog copy after password auth | State `ME9dW` button reads SIGN IN WITH GITHUB | Button text becomes SIGN IN and opens the password form in place. Design updated in this pass. | Designed |
| G22 | Settings "Auth provider" value | `8wGs5` shows GitHub | Read-only row showing "Password". Design updated in this pass. Built read-only (spec 17). | Built |
| G23 | GitHub integration page in the deferred state | `kx4iP` shows a fully connected integration | Page renders every section as skeleton with a single Alert: "GitHub connects when deployments arrive." Built at `/settings/integrations/github` (spec 18). | Built (deferred state) |
| G24 | Certificate expiry surfaces (RENEW NOW on a manager card) | State `SedYE` (04.S5) | Not rendered; no certificate lifecycle exists yet. Node cards show only the degraded and down notes (spec 13). | Designed, deferred |
| G25 | Exec connection line shows PID and TTY | Exec `f8D6u` footer | Docker's exec API does not expose them; the line shows the container name and session count only (spec 16, ADR 0026). | Missing, deferred |
| G26 | Theme setting row (system / light / dark) | Settings `8wGs5` has no appearance row | A "Theme" select as the first APPLICATION row, styled like the other rows. Built; the choice applies when saved (spec 17). | Built |
| G27 | Setup code on Create owner | Connect · Create owner `wLzP9` / `T0KgTd` has display name, email, and password only | A fourth "Setup code" field (monospace) with the help line "Printed by the controller when it starts…"; a wrong or missing code is a field error. Required so the first network client can't claim a fresh controller (ADR 0029). | Built, not in the design |

## Added in this pass

- `wwb6d` Kontainer — Connect · Sign in (dark) and `HTrey` (light)
- `wLzP9` Kontainer — Connect · Create owner (dark) and `T0KgTd` (light)
- `W8OmXO` State 02.S7 · Connect — Invalid credentials, caption `J7Hnlc`
