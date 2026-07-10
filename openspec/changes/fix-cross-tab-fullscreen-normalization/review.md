---
scale: M
full_rigor: false
execution_mode: standard
verification_mode: retained-recommended
debug_mode: standard
review_status: not-requested
delegation_mode: subagent-eligible
# code_review_mode absent — derived: M ⇒ gating-required (fail-closed)
loop_max_iterations: 40
validation_source_mode: required
spec_level: spec-anchored
doneness_mode: required
---

# Review

## Modes

| Mode | Value | Notes |
|---|---|---|
| Scale | M | Frozen intent (ad1963c): SDK bump + normalization rewrite + persistence revert + spec deltas — cross-file, single capability cluster |
| full_rigor | false | Single capability cluster; no breaking change, no migration |
| Execution Mode | standard | standard\|tdd-preferred\|tdd-required |
| Verification Mode | retained-recommended | retained-required not forced; regression evidence lands in tasks.md per intent |
| Debug Mode | standard | standard\|systematic-debugging |
| Review Status | not-requested | not-requested\|requested\|findings-received\|resolved |
| Delegation Mode | subagent-eligible | loop dispatches blind reviewers/judges via subagent adapter |
| Code Review Mode | derived (absent) | M ⇒ gating-required (fail-closed derivation) |
| Loop Max Iterations | 40 | authoring-time default for Scale M |
| Validation Source Mode | required | opsx-gates.yaml: openspec validate, wasm build, harpoon-core tests |
| Doneness Mode | required | plain M: doneness rides the code-review dispatch (blind-single-judge) |
| Spec Level | spec-anchored | |
| Model Config | (unset) | roles resolve via `opsx models`; session fallback |

## Diff Base + Worktree locator

**Diff Base SHA:** 096734ea4c80c9985debb04cf740f26a92961201
**Worktree Path:** /Volumes/Workshop/git/harpoon-opsx-fix-cross-tab-fullscreen-normalization
**Integration Branch:** main

## Manual Adjustments

- Scale M (not S): bundled SDK bump (zellij-tile 0.42.2 → 0.44.3) + behavior
  rewrite + d6a2039 persistence revert + spec deltas across four capability
  specs; intent.md is gate-required at M, which this change relies on
  (frozen baseline commit ad1963c).
- Delegation Mode subagent-eligible: review/doneness verdicts are always
  blind-dispatched per the loop discipline; implementation may run in-session.

## Execution Notes

- 2026-07-09 — review.md authored in-session during loop bootstrap
  (authored: in-session).
- 2026-07-09 — worktree created (branch opsx/fix-cross-tab-fullscreen-normalization
  from main @ 096734e); locator captured.

## Scope Expansions

<!-- One entry per evidence-gated widening; surfaced at landing or gate-green. -->

## Fidelity Round Ledger

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|
