# 0002. Single-user scope: no roles, capabilities, or recent-auth gates

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/00-goal.md

## Context

The reset doc and `docs/v2/security.md` describe Owner/Admin/Operator/Viewer roles, a capability
registry, a capabilities endpoint, and a 15-minute recent-authentication rule for destructive actions.
Several design states draw Viewer restrictions and "RE-AUTHENTICATE" buttons. Kontainer has exactly one
user, who is the Owner, and registration closes after that account exists.

## Decision

- Authorization stays "has a live Owner session". No roles table, no capability checks, no
  capabilities endpoint, no recent-auth rule.
- Destructive actions are protected by typed confirmation dialogs as drawn (Confirm Dialog `KIuxe`).
- Design states that show Viewer or re-authentication behaviour are not built. The screens render
  their Owner variant only.
- The `principals.kind`, `authorization_epoch`, and the `service` principal kind stay unused and
  untouched.

## Consequences

- Every mutation handler checks only the session middleware. Adding roles later means adding one check
  at the HTTP edge per action, which the action-per-file layout keeps cheap.
- The 15-minute rule can be added inside the confirm-dialog flow without touching the actions.
- This ADR must be revisited before a second user is ever created.
