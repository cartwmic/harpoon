---
# Machine-readable mode block — the SOLE source opsx gate reads (it never parses
# the prose table below). Keep the table in sync as the human-facing mirror.
scale: M
full_rigor: false
execution_mode: standard
verification_mode: retained-recommended
debug_mode: standard
review_status: not-requested
delegation_mode: single-agent
# code_review_mode: derived when absent (M ⇒ gating-required)
loop_max_iterations: 40
validation_source_mode: required
spec_level: spec-anchored
doneness_mode: required
---

<!-- authored: in-session -->

# Review

## Modes

| Mode | Value | Notes |
|---|---|---|
| Scale | M | Frozen intent.md recommends Scale M, `full_rigor: false` — three related defects (F1 race, F2 prune-guard, F3 restore hardening), two capability deltas, core+shim+script+README surface; owner confirmed M at explore wrap |
| full_rigor | false | Per intent recommendation and owner pick — no cross-capability breakage/migration |
| Execution Mode | standard | |
| Verification Mode | retained-recommended | |
| Debug Mode | standard | |
| Review Status | not-requested | |
| Delegation Mode | single-agent | |
| Code Review Mode | derived (absent) | M ⇒ gating-required (derived fail-closed) |
| Loop Max Iterations | 40 | Authoring-time default for M |
| Validation Source Mode | required | Committed tmux-hosted regression script `scripts/toggle-pipe-regression.sh` (extended scenarios) is the agent-independent validation source (precedent: pipe-toggle-invocation) |
| Doneness Mode | required | Plain-M ⇒ doneness rides the code-review dispatch (designated reviewer) |
| Spec Level | spec-anchored | Default; owner pick at explore wrap |
| Model Config | (unset) | Roles resolve via `opsx models`; session model fallback |

## Diff Base + Worktree locator

**Diff Base SHA:** 7409760b3d57278a66b45dddf66c49ce01269515
**Worktree Path:** /Volumes/Workshop/git/harpoon--opsx-respawn-state-handoff
**Integration Branch:** main

<!-- Diff Base = main HEAD at proposal authoring (intent freeze commit).
Re-confirm merge-base at worktree creation; if main advanced, update BEFORE
first implementation commit — immutable afterwards. Worktree is created at
apply start (worktree-always model). -->

## Manual Adjustments

- Scale M / full_rigor false / spec-anchored adopted directly from frozen
  intent.md and the owner's explicit picks at the explore→propose handoff —
  no deviation.
- design.md deliberately NOT authored (plain-M decision-gated): every
  non-trivial trade-off (hand-off mechanism, rejected alternatives, Q1–Q3)
  is already settled in the FROZEN intent.md Decision record and
  proposal.md ## Open Questions; a design.md would restate frozen content
  and add a fidelity dispatch with no new decision surface.

## Execution Notes

<!-- Transient observations appended during apply. One-line entries when a
non-trivial decision is made mid-task. Durable knowledge → retrospective.md. -->

## Fidelity Round Ledger

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|

<!-- Design-bearing changes only; this change carries no design.md, so this
ledger stays empty unless a design.md is later authored under a re-ruling. -->

## Code Review Round Ledger

| Round | Verdict | P0 | P1 | Reviewers | Attested HEAD |
|---|---|---|---|---|---|
