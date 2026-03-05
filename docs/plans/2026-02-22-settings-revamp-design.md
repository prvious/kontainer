# Settings Page Revamp

## Problem

The current settings page uses horizontal tabs with Card wrappers per section. This feels like a web SaaS dashboard, not a native desktop app. Devops users expect a denser, more utilitarian layout.

## Design Decisions

- **Layout:** Full-page vertical scroll with grouped sections separated by headings
- **Tone:** Industrial / utilitarian — compact, dense, monospaced section labels, thin separators
- **Save behavior:** Immediate apply (no save button), except Connection tab where URL changes require explicit Save

## Structure

Replace `Tabs` + `Card` wrappers with a single scrollable page:

```
Settings
Application and cluster configuration

GENERAL
─────────────────────────────────────────────
Refresh interval                     [ 5s ▾ ]
How often data is polled from the agent

APPEARANCE
─────────────────────────────────────────────
Font                               [ Geist ▾ ]
Typeface used across the app UI

TERMINAL
─────────────────────────────────────────────
Default shell                      [ Auto  ▾ ]
Fallback shell when none is specified
─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
Font size                          [ 14    ▾ ]
Terminal text size in pixels
─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
Font family                [ JetBrains Mono  ]
CSS font-family for the terminal
─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
Scrollback buffer                  [ 1,000 ▾ ]
Lines kept in scroll history

CONNECTION
─────────────────────────────────────────────
Display name               [ Production     ]
A friendly name for this cluster
─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─
Agent URL                  [ https://...     ]
Endpoint for this cluster's agent
                                      [ Save ]
```

## Visual Specs

### Section Headings
- `text-xs font-mono uppercase tracking-widest text-muted-foreground`
- Followed by a full-width `border-b border-border` separator

### Setting Rows
- `flex items-center justify-between py-3`
- Left side: Label (`text-sm font-medium`) + description (`text-xs text-muted-foreground`)
- Right side: Control (Select, Input, Switch) — compact, right-aligned
- Rows within a section separated by a subtle `border-b border-border/50` (dashed or faint)

### Page Container
- `max-w-2xl mx-auto px-6 py-6` — centered, constrained width for readability
- Scrolls within the existing `<main>` overflow area

### Controls
- Selects: `w-36` for short values, `w-48` for longer values
- Inputs: `w-64` for text fields
- All controls use existing shadcn components (no Card wrappers)

## What Changes

### Files Modified
- `app/src/routes/_authenticated/settings/index.tsx` — Remove Tabs, inline all sections
- `app/src/components/settings/general-tab.tsx` — Remove Card wrapper, export section content
- `app/src/components/settings/appearance-tab.tsx` — Remove Card wrapper, export section content
- `app/src/components/settings/terminal-tab.tsx` — Remove Card wrapper, export section content
- `app/src/components/settings/connection-tab.tsx` — Remove Card wrapper, keep Save button, export section content

### Files NOT Modified
- No new components needed — this is a simplification, not an addition
- No shadcn components to install
- No hook changes

## What Stays the Same
- All existing hooks (`useSettings`, `useAppearance`, `useAuth`)
- All existing controls and their behavior
- Connection tab Save button (explicit save for URL changes)
- Page title and subtitle
