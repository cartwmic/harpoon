# Code Review

**Change:** fix-cross-tab-fullscreen-normalization
**Verdict:** fail
**review_mode:** adversarial-multimodel
**reviewer-provenance:** pi-subagents parallel dispatch — openai-codex/gpt-5.6-sol (designated doneness judge), claude-bridge/claude-fable-5
**Diff Base SHA:** 096734ea4c80c9985debb04cf740f26a92961201
**Reviewed Range:** 096734ea4c80c9985debb04cf740f26a92961201..ca77e20fc2af067b8822fe0eefe3efc53a27c0ba
**Attested HEAD:** ca77e20fc2af067b8822fe0eefe3efc53a27c0ba
**Baseline:** intent.md + proposal + specs + plan + tasks status
**Generated:** 2026-07-09

## Round tracker

| Round | Mode | P0 | P1 | P2 | P3 | Reviewer verdicts | Reviewed HEAD |
|---|---|---|---|---|---|---|---|
| 1 | blind | 0 | 4 | 1 | 3 | gpt-5.6-sol:fail fable-5:fail | ca77e20fc2af067b8822fe0eefe3efc53a27c0ba |

## Findings

Consolidated round 1 (max across reviewers per severity; findings files
/tmp/cr-r1-sol.md, /tmp/cr-r1-fable.md are the verdict sources).

| # | Finding | Severity | Status |
|---|---|---|---|
| 1 | Constitution I: toggle decision returns bare bool; shim sequences host calls directly instead of core-emitted target-bearing effects (pipe_api.rs, main.rs jump path) | P1 | open |
| 2 | Mandatory intent scenario 2 not cold-start in harness: S2 reuses S1's warm persistent pipe instance; cold × target-is-fullscreen-pane quadrant never exercised cold | P1 | open |
| 3 | Harness loads() emits "0\n0" on fresh log (grep -c prints 0 AND exits 1, || echo 0 double-fires); numeric compare breaks — repeatability violation | P1 | open |
| 4 | Permission seeding skipped when permissions.kdl absent — fresh env blocks on interactive prompt; violates repeatable-harness task contract | P1 | open |
| 5 | Per-scenario regression evidence not recorded in the mandated artifacts (intent: tasks.md; T3.2: review.md Execution Notes had it integration-side only, not per worktree artifacts reviewers see) | P1→consolidated under P1 count above | open |
| 6 | Harness fullscreen detection keys on '││' glyph; theme-fragile | P2 | open |
| 7 | T5.1 claims three AC-citing test literals; targeted-pipe-delivery is docs-verified only (tasks/plan inconsistency) | P3 | open |
| 8 | apply_effects doc comment attached to jump_focus_fullscreen; apply_effects undocumented | P3 | open |
| 9 | Core fn deviates from task letter (no pane id param, bool return) — superseded by fix of #1 | P3 | open |

## Applied fixes

- (round 2 pending)

## Residual risks

- Pipe arriving while harpoon UI pane visible+focused resolves Unknown → no
  toggle (spec-sanctioned carve-out); routed to follow-ups.
- zellij-tile names the stable tab id `focused_tab_index`; naming trap on
  future SDK upgrades — routed to follow-ups.

## Verdict rationale

Round 1: both blind reviewers fail. Normalization logic, SDK bump, hide_self
revert, and docs judged baseline-faithful by both; open P1s concentrate on
Constitution I effect-split shape and regression-harness contract gaps
(cold S2, fresh-log counter, fresh-env permissions, evidence placement).
Verdict: pass requires P0+P1 = 0 — not met.
