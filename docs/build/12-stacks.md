# 12. Stacks

## Goal

Stacks derived from service labels, with the removal plan and confirmation. Deploy stack is
deferred (Compose is out of scope by the reset doc); the button renders disabled.

Design: Stacks `twXyi` (light `eOqXg`); states `Y1Nv0` (04.S1 empty), `Flhrm` (04.S2 remove
confirmation); captions `zh11l`, `n3HebB`. Gaps G3 (deploy), G9 (stack detail → filter services).

## Server

- `list_stacks` (controller, from the manager engine's services and the containers fan-out):
  group services by `com.docker.stack.namespace`; per stack: name, service count, containers
  `running / desired` (sum of task counts), CPU percent and memory summed from container stats
  whose stack label matches, `lastDeployed` = max service `updated`, status `running` (every
  service converged), `partial`, `stopped` (no running task).
- `GET /api/v1/stacks` → the list, `{ items }` with no `snapshotAt` (ADR 0040).
  `GET /api/v1/stacks/{name}/plan` → removal preview:
  `services: [names]`, `networks: [names with the stack label]`, `volumesRetained: [named volumes
  referenced by service mounts]`.
- `DELETE /api/v1/stacks/{name}` → 202 `stack.remove`. Executor: remove every service, wait until
  their tasks are gone (poll 2 s, up to 60 s), then remove stack-labelled networks (retry while
  `in_use`, up to 60 s), never touch volumes; detail `{ phase, servicesRemoved, networksRemoved,
  volumesRetained }`.

## Desktop

- Route `/stacks`. Tauri: `list_stacks`, `get_stack_removal_plan`, `remove_stack`. Hook
  `useStacks()`.
- Header: `STACKS`, subtitle `N stacks deployed · S services · C containers`, DEPLOY STACK
  disabled with tooltip "Not built yet" (ADR 0010).
- TableToolbar: search; Sort: name, last deployed.
- DataTable: STATUS (`running` success, `partial` warning, `stopped` destructive), STACK NAME
  (no compose file subtitle; Docker doesn't keep it), SERVICES, CONTAINERS `running/desired`, CPU,
  MEMORY, LAST DEPLOYED, actions `⋮`: Show services (navigates to `/services?stack=<name>`, G9),
  Remove.
- Empty (04.S1): table header stays with count 0, toolbar hidden, EmptyState explains that stacks
  appear when a Compose file is deployed with `docker stack deploy`, primary action DEPLOY STACK
  disabled, secondary "Docker docs" opens the stack deploy documentation with the opener plugin.
- Remove (04.S2): ConfirmDialog opened from the row menu, loading the plan first: services to be
  removed, stack-owned networks to be removed, named volumes retained (listed so no data is lost
  by surprise). Typing the stack name enables REMOVE STACK. No re-authentication step (ADR 0002).
  While running, the dialog shows the operation phase; on success the row disappears and a toast
  reports counts.

## Tests

- Controller: grouping and status math with fake services and containers; plan lists volumes
  from mounts; executor order and retries with a fake manager engine.
- Real cluster: the three seed stacks list with correct counts; remove `toolbox` and see its
  services and network go while volumes stay; redeploy it with `dev/seed-services.sh` afterwards.
- Desktop: empty state; plan rendering; typed confirmation; menu navigation to services.

## Done when

- Removing a seed stack from the desktop shows the plan, requires the typed name, runs with a
  visible phase, and leaves volumes; all checks pass; `PROGRESS.md` updated.
