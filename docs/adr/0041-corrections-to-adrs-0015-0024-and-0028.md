# 0041. Correct factual errors in ADRs 0015, 0024 and 0028

- Status: accepted
- Date: 2026-10-04
- Spec: none
- Amends: ADR 0015, ADR 0024, ADR 0028

## Context

The PR 135 review found three accepted ADRs that state things the code doesn't do. Accepted ADRs
are never rewritten, so this ADR records the corrections. Readers of 0015, 0024 and 0028 should read
this one alongside them.

## Decision

- **ADR 0015, legacy check-ins.** It says a pre-sampling check-in (fields 1–3 only) "still decodes
  and records the node without a sample". It decodes, but it is not recorded:
  - `nodes::domain::validate` requires a valid `advertise_address`, so the check-in is rejected with
    `InvalidArgument`.
  - The rejection is correct. The controller can't dial an agent without its address, and no agent
    older than the membership spec was ever deployed.
- **ADR 0024, outage detection bound.** It says an outage shows "about 17 s after the last good
  probe at worst". That holds only when the probe fails fast (connection refused). A controller
  that stops answering makes each attempt wait out its 2 s deadline, so the worst case is about
  23 s: the 15 s interval, plus three 2 s attempts, plus two 1 s retry delays.
  - Stopping the controller, which is spec 14's "Done when" case, refuses connections and stays
    within its 20 s.
- **ADR 0028, the breadcrumb.** It says the Shadcn `breadcrumb` item needs an "unknown" npm package
  `cn`, and that adding it would be a supply-chain risk. That is wrong:
  - `cn` is the class-merge package ADR 0013 approved.
  - Five files in `app/src/components/ui/` already import it.
  - The kit breadcrumb stays for now. Swapping it for the Shadcn component is a CLAUDE.md-only
    change with no runtime effect, deferred until that code is next touched.
- **PROGRESS row 18.** Its "all exit 0" counted `pnpm fmt` and `cargo fmt` in write mode, which
  can't fail. 00-goal rule 8 and CLAUDE.md now list `pnpm --filter app fmt:check` and
  `cargo fmt --check` as the checks.

## Consequences

- No code changes. The ADRs keep their decisions; only the facts above are corrected.
- The ledger's row 18 carries a correction note pointing here.
