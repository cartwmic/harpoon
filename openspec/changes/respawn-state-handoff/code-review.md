# Code Review

**Change:** respawn-state-handoff
**Verdict:** fail
**review_mode:** adversarial-multimodel
**reviewer-provenance:** openai-codex/gpt-5.6-sol + claude-bridge/claude-fable-5 (blind via pi-subagents delegate)
**Diff Base SHA:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4
**Reviewed Range:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4..d731bf6e4fcfabdee9bac01511afd01ff5a49c3a
**Attested HEAD:** d731bf6e4fcfabdee9bac01511afd01ff5a49c3a
**Baseline:** intent.md + proposal + specs + plan + tasks status + constitution/domain (design absent by plain-M decision gate)
**Generated:** 2026-07-13

## Verdict contract (embed in every reviewer dispatch prompt)

FAIL only for a frozen-baseline violation or objective correctness/security
defect. P0/P1 gate; P2/P3 are advisory. Verdict pass iff no P0/P1 remains
open.

## Round tracker

| Round | Mode | P0 | P1 | P2 | P3 | Reviewer verdicts | Reviewed HEAD |
|---|---|---|---|---|---|---|---|
| 1 | blind | 0 | 7 | 3 | 4 | gpt-5.6-sol:fail claude-fable-5:fail | d731bf6e4fcfabdee9bac01511afd01ff5a49c3a |

## Findings

<!-- Counts above are max-across-reviewers per severity, with no
cross-reviewer finding matching. Full sole-source findings files:
/tmp/rsh-r1-sol.md and /tmp/rsh-r1-fable.md. Rows retain source identity;
similar findings are not merged for counting. -->

| # | Finding | Severity | Status |
|---|---|---|---|
| sol-1 | Successor can render before bootstrap, then adoption clears panes without resolving cached manifest — violates first-render full-target guarantee. | P1 | open |
| sol-2 | Bootstrap can prevent disk-load initiation (session name set first), and adopted late disk result is ignored rather than reconciled. | P1 | open |
| sol-3 | Save readiness treats any non-empty pane event + bootstrap adoption as resolved-disk + full-manifest; partial-manifest prune hole remains. | P1 | open |
| sol-4 | Unknown persisted baseline classifies every candidate as shrinking, blocking additive/reorder saves contrary to delta AC. | P1 | open |
| sol-5 | Permissions are tracked as one bool; bootstrap-send denial cannot still take spawn→disk-fallback, and other gated CLI calls remain unguarded. | P1 | open |
| sol-6 | Spawn returning None shows/keeps predecessor instead of the delta scenario's close + successor disk fallback. | P1 | open |
| sol-7 | Regression does not force first-render state, partial-manifest prune window, or permission-denied degradation. | P1 | open |
| fable-1 | Permission-denied degrade regression promised by frozen intent/tasks/plan is omitted while task is checked complete. | P1 | open |
| fable-2 | Adoption skips immediate cached-manifest restore, leaving an empty/resolving render window. | P2 | open |
| fable-3 | Tasks/plan still describe superseded readiness-based duplicate guard; implementation/spec now use pipe identity. | P2 | open |
| fable-4 | Spawn-None delta wording conflicts with status-quo show-in-place behavior; split from non-plugin-id case. | P2 | open |
| fable-5 | Unknown-baseline additive save is deferred; spec currently says all additive saves proceed. | P3 | open |
| fable-6 | MergeMissing can resurrect a user-deleted disk bookmark in the tiny no-bootstrap pre-load window. | P3 | open |
| fable-7 | `manifest_seen` is a non-empty proxy, not a defined full-manifest condition. | P3 | open |
| fable-8 | Bootstrap-before-stale-toggle ordering relies on probe ordering; theoretical reverse order remains. | P3 | open |

## Applied fixes

- None — round 1 findings sealed; fixes land after this verdict-only commit.

## Residual risks

- Native core tests: 259 green. Scenario scripts were not run by reviewers
  (read-only/global-state rule). No gate/validation manifest touched.

## Verdict rationale

FAIL. Both valid blind reviewers found open P1 baseline/correctness gaps.
Round is converging once change-scoped implementation/spec/evidence fixes land;
then full-diff blind round 2 is required.
