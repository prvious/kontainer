---
name: service-resources-ui
description: Resource editor UI in service detail view showing limits/reservations with inline editing
depends_on: update-service-resources
---

## Feature Description

Add a "Resources" tab to the service detail page that displays current CPU and memory limits/reservations and allows inline editing. Shows a clear visual layout of configured vs unconfigured resources, with human-readable formatting (e.g. "0.5 CPU", "512 MiB"). Follows the same edit-in-place pattern used by the Environment and Image editors.

## Implementation Plan

### Frontend Components

**New component: `app/src/components/resource-editor.tsx`**

A card-based editor with four resource fields in a 2x2 grid layout:

```
┌─────────────────────────────────────────────┐
│ Resources                          [Edit]   │
├──────────────────────┬──────────────────────┤
│ CPU Limit            │ Memory Limit         │
│ 0.5 cores            │ 512 MiB              │
├──────────────────────┼──────────────────────┤
│ CPU Reservation      │ Memory Reservation   │
│ 0.25 cores           │ 256 MiB              │
└──────────────────────┴──────────────────────┘
```

Display mode:
- Each resource shows its current value or "No limit" / "No reservation" when unset
- Values formatted with helper functions: CPU as cores with up to 3 decimals, memory as MiB/GiB
- Visual indicator (muted text) for unset values

Edit mode (triggered by Edit button):
- CPU fields: number input with step 0.1, placeholder "e.g. 0.5", suffix "cores"
- Memory fields: number input with a unit dropdown (MiB / GiB), auto-converts to bytes for submission
- Empty input = clear the limit (sets to 0)
- Save and Cancel buttons replace the Edit button
- Validation: inline error messages for invalid values (CPU < 0.001, memory < 4 MiB, reservation > limit)

Shadcn components to use:
- `Card`, `CardContent`, `CardHeader`, `CardTitle` — container
- `Button` — edit/save/cancel actions
- `Input` — number inputs for CPU and memory values
- `Select` — memory unit dropdown (MiB / GiB)
- `Badge` — display current values
- `Separator` — between limits and reservations sections

Icons from lucide-react:
- `Cpu` — CPU fields
- `MemoryStick` — Memory fields
- `Pencil` — Edit button
- `Check`, `X` — Save/Cancel buttons

**Helper functions (add to `app/src/lib/format-utils.ts` or similar):**

```typescript
// Format bytes to human-readable
export function formatBytes(bytes: number): string {
    if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GiB`;
    return `${(bytes / 1048576).toFixed(0)} MiB`;
}

// Format CPU cores
export function formatCPU(cores: number): string {
    return `${cores} cores`;
}

// Parse memory input + unit to bytes
export function parseMemoryToBytes(value: number, unit: "MiB" | "GiB"): number {
    return unit === "GiB" ? value * 1073741824 : value * 1048576;
}
```

**Update service detail page: `app/src/routes/_authenticated/services/$serviceId.tsx`**

Add a new "Resources" tab after "Environment":

```tsx
<TabsTrigger value="resources">Resources</TabsTrigger>

<TabsContent value="resources">
    <ResourceEditor
        serviceId={serviceId}
        resources={data.resources}
    />
</TabsContent>
```

### State Management

The ResourceEditor manages its own local edit state:
- `isEditing` boolean — toggles between display and edit mode
- `formState` object — holds draft values during editing
- On save: calls `useUpdateServiceResources` mutation, exits edit mode on success
- On cancel: discards draft, exits edit mode
- Loading/error states from the mutation shown inline (spinner on Save button, toast or inline error on failure)

## Acceptance Criteria

- [ ] "Resources" tab visible in service detail view
- [ ] Displays current CPU/memory limits and reservations when set
- [ ] Shows "No limit" / "No reservation" for unset values
- [ ] CPU displayed as cores (e.g. "0.5 cores")
- [ ] Memory displayed as MiB or GiB with appropriate unit
- [ ] Edit mode shows number inputs for all four fields
- [ ] Memory edit has MiB/GiB unit selector
- [ ] Empty field on save clears that resource constraint
- [ ] Inline validation prevents submitting invalid values
- [ ] Save button shows loading state during mutation
- [ ] Error from backend displayed to user
- [ ] Successful save exits edit mode and shows updated values
- [ ] Works correctly when service has no existing resource config (all null)
- [ ] All Shadcn UI components used (no custom UI primitives)

## Testing Strategy

### Frontend Tests

**New file: `app/tests/unit/resource-editor.spec.tsx`**

Component tests:
- Renders display mode with all four resource values
- Renders "No limit" for null CPU/memory limits
- Renders "No reservation" for null reservations
- Formats memory correctly (MiB for < 1 GiB, GiB for >= 1 GiB)
- Formats CPU correctly (decimal cores)
- Clicking Edit switches to edit mode with inputs
- Edit mode pre-fills current values in inputs
- Clicking Cancel reverts to display mode without changes
- Clicking Save calls `update_service_resources` invoke with correct params
- Save with empty CPU field sends `cpuLimit: 0` (clear)
- Shows loading state on Save button during mutation
- Shows error message when mutation fails
- Exits edit mode after successful save
- Validates CPU minimum (< 0.001 shows error)
- Validates memory minimum (< 4 MiB shows error)
- Validates reservation does not exceed limit

**Update file: `app/tests/unit/service-detail.spec.tsx`**

- Add "Resources" tab to the rendered tabs
- Verify Resources tab renders ResourceEditor when clicked

## Code Formatting

- TypeScript: `pnpm fmt` (oxfmt)

## Additional Notes

- The edit pattern mirrors `EnvEditor` closely: display card → edit mode with inputs → save/cancel. This keeps the UX consistent across the service detail tabs.
- Memory unit auto-detection: When displaying, use GiB if value >= 1 GiB, otherwise MiB. When editing, default the dropdown to the same unit as the current display.
- For services that have never had resource limits set (`resources: null`), all four fields show as "No limit" / "No reservation" in display mode and start empty in edit mode.
- Consider adding a subtle info tooltip explaining the difference between limits (hard cap, container killed if exceeded) and reservations (guaranteed allocation, used for scheduling).
