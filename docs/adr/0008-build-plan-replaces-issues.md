# 0008. The build plan in docs/build replaces GitHub issues for this phase

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/00-goal.md

## Context

The reset doc made GitHub Issues the execution source of truth: one parent epic, fifteen domain
epics, and vertical leaf issues, each delivered as its own pull request. After seven weeks that
produced 24 closed issues and a read-only walking skeleton. The remaining 50 open issues were written
before the desktop redesign, which now defines the product feature by feature. The owner wants one
autonomous agent to build the whole desktop scope overnight without waiting on review per issue.

## Decision

- `docs/build/` is the execution source of truth: `00-goal.md` (standing instructions and the
  `/goal` condition), numbered feature specs, `design-gaps.md`, and `PROGRESS.md` as the ledger.
- `docs/adr/` records decisions, following the template in its README. The reset doc remains the
  architecture authority; ADRs extend it and say so when they deviate.
- All open GitHub issues and the parent epic are closed with a comment pointing here. Milestones
  stay as history. PR #70 (documentation only) is closed unmerged.
- Work happens on one branch, `build/desktop-v1`, with commits per coherent unit and no pull
  requests until the owner reviews.
- Issues return when more than one person works on the repo, or when a feature needs external
  scheduling visibility.

## Consequences

- Review shifts from per-PR to per-ADR and per-`PROGRESS.md` row, which is what one owner can
  actually keep up with.
- Historical issue bodies link to documents removed on 2026-09-28; git history keeps them.
