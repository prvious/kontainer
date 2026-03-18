---
name: frontend-ui
description: Use when working on the React frontend UI, especially when choosing or composing Shadcn UI components and Tailwind CSS styles
---

# Frontend UI

Use this skill for UI work in `app/`.

## Core Rules

- Prefer existing app components and patterns before creating something new.
- Use Shadcn UI whenever an equivalent component exists.
- If a needed Shadcn component is missing, install it with the Shadcn CLI before building around it.
- Never manually edit files in `app/src/components/ui/`; they are Shadcn-managed.
- Follow the surrounding visual language instead of introducing a new one.

## Shadcn Guidance

- Check whether a component already exists in `app/src/components/ui/` before installing another one.
- Compose screens from Shadcn primitives first, then add project-specific wrappers only when reuse is clear.
- If a Shadcn component exposes a `render` prop pattern in this codebase, follow the local pattern instead of forcing `asChild`.

## Tailwind Guidance

- Follow existing Tailwind conventions in nearby files before introducing new utility combinations.
- Prefer layout utilities such as `flex`, `grid`, `gap`, and container padding over ad hoc margins.
- Keep class lists intentional: remove redundant utilities, avoid conflicting classes, and put shared styling on the highest sensible parent.
- Preserve existing dark mode behavior where the surrounding page or component already supports it.
- Use the project's Tailwind v4 patterns already present in the codebase.

## Workflow

1. Read sibling components to match structure, spacing, typography, and state handling.
2. Reuse or install the right Shadcn primitive.
3. Implement styling with local Tailwind conventions.
4. Verify the result on both desktop and mobile-sized layouts when the UI is responsive.

## Related Skills

- Use `vitest-tauri-testing` when frontend changes also require Tauri-aware Vitest coverage.
