# 0036. Typed confirmation when data can be lost

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/00-goal.md (rule 13), docs/build/07-containers.md
- Amends: ADR 0020
- Decision record: owner decision D2 (option B) in the PR 135 review

## Context

Rule 13 of `docs/build/00-goal.md` put "destructive actions behind typed confirmation". The specs
then carved out exceptions, and the build ended up inconsistent:

- Volumes asked for the typed name: ADR 0019 upgraded spec 09's plain dialog, citing rule 13.
- Stopped containers and empty networks got a plain Cancel / Remove: specs 07 and 10 asked for it.

ADR 0020 justified the plain network dialog as "reversible (recreate it)". That is false. A
recreated network gets a new ID, and a different subnet unless one was fixed, so anything that
referenced the old ID breaks. A false premise like that could justify weaker confirmations later.

What each removal loses:

- **Container, in any state:** its writable layer (every file written outside a volume), its logs,
  and its configuration. None of it can be recovered.
- **Volume:** the data itself.
- **Empty network:** no data; only its identity.

## Decision

- **Rule 13 now reads:** destructive actions that can lose data require typed confirmation; others
  use a plain confirm. A reviewer checks one question: can this lose data?
- **Containers:** removal asks for the typed name in every state, stopped included. `needsForce`
  (running, paused or restarting) now decides only whether the removal passes `force`.
- **Unchanged:**
  - a volume asks for its name (ADR 0019);
  - an image used by containers, and a stack, ask for typed confirmation (specs 08 and 12);
  - bulk Stop and Remove ask for the typed count (spec 07).
- **Empty networks keep the plain Cancel / Remove.** The reason is that they hold no data, not
  that removal is reversible. This corrects ADR 0020's "That's reversible (recreate it), unlike a
  volume's data."

## Consequences

- Removing a stopped container takes one more step: typing its name.
- Spec 07's Remove line and design gap G7 describe the typed name for every state.
- A future destructive action follows the same test. If it can lose data, it is typed; if not,
  a plain confirm is enough.
