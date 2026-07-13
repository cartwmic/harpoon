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

**Diff Base SHA:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4
**Worktree Path:** /Volumes/Workshop/git/harpoon--opsx-respawn-state-handoff
**Integration Branch:** main

<!-- Diff Base confirmed at worktree creation = main HEAD (propose commit);
immutable from here. -->

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

- 2026-07-13 — Task 1.1: spike evidence preserved from /tmp into
  evidence/ (log excerpt confirms spawn-id return, 119ms pre-grant
  bootstrap delivery, ~380ms same-uuid CLI pipe re-delivery).
- 2026-07-13 — Tasks 2.1–2.4 (core, 259 native tests green): duplicate
  tolerance mechanism CORRECTED from the propose-time sketch — probe
  timing proves the re-delivered pipe arrives AFTER the menu is shown, so
  the spec's "readiness after first shown render" parenthetical could not
  catch it; amended spec/proposal (Q3) to deterministic pipe-identity
  matching (payload carries sender-handled CLI pipe id, one-shot ignore,
  client still released). Keybind-sourced pipes carry no id and are never
  suppressed (re-delivery is a CLI-pipe blocking-mechanism behavior).
- 2026-07-13 — Assumption: adopted bootstrap counts as the disk baseline
  for the destructive-save guard (sender's disk verified current at send
  time); disk reconcile after early user mutation = MergeMissing (disk
  entries absent from memory append with index=None), never clobber.
- 2026-07-13 — Apply validation at worktree `d731bf6`: core native tests
  259/259 green; wasm32-wasip1 release build clean; strict OpenSpec valid;
  shell syntax clean; expanded tmux-hosted scratch regression 19/19 green
  (S8 immediate target hand-off, S9 same-uuid duplicate tolerance + CLI
  release, S10 persisted-list non-shrink across respawns). Permission-denied
  end-to-end remains intentionally non-scripted: unseeded permission opens
  an interactive prompt the harness cannot answer; grant gating + pure core
  decisions cover the degrade path.

## Fidelity Round Ledger

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|

<!-- Design-bearing changes only; this change carries no design.md, so this
ledger stays empty unless a design.md is later authored under a re-ruling. -->

## Code Review Round Ledger

| Round | Verdict | P0 | P1 | Reviewers | Attested HEAD |
|---|---|---|---|---|---|
