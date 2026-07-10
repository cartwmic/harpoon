---
# Machine-readable mode block — the SOLE source opsx gate reads (it never parses
# the prose table below). Keep the table in sync as the human-facing mirror.
scale: S
full_rigor: false
execution_mode: standard
verification_mode: retained-recommended
debug_mode: standard
review_status: not-requested
delegation_mode: single-agent
# code_review_mode: derived when absent (S ⇒ advisory)
loop_max_iterations: 20
validation_source_mode: required
spec_level: spec-anchored
doneness_mode: required
---

# Review

## Modes

| Mode | Value | Notes |
|---|---|---|
| Scale | S | Frozen intent.md recommends Scale S, `full_rigor: false` (single-file permission-request fix + spec delta + regression scenario) |
| full_rigor | false | Per intent recommendation |
| Execution Mode | standard | |
| Verification Mode | retained-recommended | |
| Debug Mode | standard | |
| Review Status | not-requested | |
| Delegation Mode | single-agent | |
| Code Review Mode | derived (absent) | S ⇒ advisory (derived fail-closed) |
| Loop Max Iterations | 20 | Authoring-time default for S |
| Validation Source Mode | required | Committed regression scenario script is the agent-independent validation source |
| Doneness Mode | required | Template default retained |
| Spec Level | spec-anchored | Default |
| Model Config | (unset) | Roles resolve via `opsx models`; session model fallback |

## Diff Base + Worktree locator

**Diff Base SHA:** 4bb20ed7d34b74ac2e11690410a8e48a191d01fa
**Worktree Path:** /Volumes/Workshop/git/harpoon--opsx-request-read-cli-pipes-permission
**Integration Branch:** main

## Manual Adjustments

- Scale S adopted directly from frozen intent.md ("Recommended Scale: S,
  `full_rigor: false`") — no deviation; autonomous loop recorded this as an
  assumption rather than pausing to confirm.

## Execution Notes

- 2026-07-10 — review.md authored by openspec-loop (earliest GATE-FAIL:
  review.md absent / Scale unparseable).
- 2026-07-10 — worktree ensured (`opsx worktree ensure`); locator captured
  (Diff Base 4bb20ed, branch opsx/request-read-cli-pipes-permission).

## Scope Expansions

<!-- Evidence-gated widenings. One entry per widening. -->

## Fidelity Round Ledger

<!-- Append-only; one row per sealed design-fidelity round. Change is not
design-bearing at Scale S (design.md skipped), so no rows expected. -->

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|
