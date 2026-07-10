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
loop_hold: true
loop_hold_reason: "gate green at worktree 1303cee — ready-to-archive report presented; awaiting human archive authorization"
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
- 2026-07-09 — T3.2 regression evidence: scripts/fullscreen-regression.sh run
  twice consecutively at worktree 5f35569, 7/7 assertions PASS both runs —
  S1 cold-start hidden-target lands fullscreen (+ new-load proof of coldness);
  S2 fullscreened-target-itself stays fullscreen; S3 warm cross-tab lands
  fullscreen (+ zero-new-load proof of instance persistence); S4 5×
  hide/relaunch under fullscreen terminal re-shows focused with zero new
  loads (quirk detector). One earlier run had a transient S1 timing flake
  before marker-format fix; post-fix runs stable.
- 2026-07-09 — assumption recorded: local zellij exports $ZELLIJ_PANE_ID as
  bare N (not terminal_N); harness accepts both forms, mirroring
  parse_pane_id.
- 2026-07-09 — round-1 blind review (gpt-5.6-sol + fable-5, both fail):
  0 P0 / 4 P1 consolidated; fixes landed at worktree 1303cee (core effect
  plan, cold S2 via distinct-path wasm identity, loads() normalization,
  fresh-env permission seeding, evidence into tasks.md). Post-fix harness:
  8/8 assertions PASS × 2 consecutive runs (S1 cold hidden-target, S2 COLD
  fullscreened-target-itself, S3 warm cross-tab zero-load, S4 quirk
  detector zero-load). Advisories routed to follow-ups.md.
- 2026-07-09 — round-2 blind review (same models): quiet round, 0 P0/P1;
  both pass at 1303cee. code-review.md sealed pass
  (adversarial-multimodel), doneness.md sealed satisfied
  (blind-single-judge, designated reviewer gpt-5.6-sol). opsx gate
  --worktree: GATE-PASS (M), exit 0. Loop landed: ready to archive,
  awaiting human authorization.
- 2026-07-09 — archive authorized by user. archive-check round 1 refused
  (stale base); remedy applied: branch rebased onto main d930ab1 (new HEAD
  1e99770), round-3 blind 2-model re-attest (quiet pass), verdicts re-sealed
  (eeb126e). GATE-PASS + archive-check OK. AC↔test gate:
  pane-pipe-api.targeted-pipe-delivery has no test literal by design —
  documentation-only AC, verified via the README --plugin grep (plan step 5,
  T5.1); allowed per option B. verify.md absent — allowed
  (retained-recommended). Branch ff-merged to main; worktree removed.

## Scope Expansions

<!-- One entry per evidence-gated widening; surfaced at landing or gate-green. -->

## Fidelity Round Ledger

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|
