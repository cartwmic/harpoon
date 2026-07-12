# Code Review

**Change:** pipe-toggle-invocation
**Verdict:** fail
**review_mode:** blind
**reviewer-provenance:** openai-codex/gpt-5.6-sol, claude-bridge/claude-fable-5
**Diff Base SHA:** 402ac1e2024982a72615203a11bfb3d5ff42311d
**Reviewed Range:** 402ac1e2024982a72615203a11bfb3d5ff42311d..a8a1c25e1bdc8a2991ad02324d3214e105deb8ff
**Attested HEAD:** a8a1c25e1bdc8a2991ad02324d3214e105deb8ff
**Baseline:** intent.md + proposal + specs + design + plan + tasks status
**Generated:** 2026-07-11

## Verdict contract

Baseline-bounded review: fail only for a frozen-baseline violation or an
objective correctness/security defect. P0/P1 findings gate; P2/P3 do not.

## Round tracker

| Round | Mode | P0 | P1 | P2 | P3 | Reviewer verdicts | Reviewed HEAD |
|---|---|---|---|---|---|---|---|
| 1 | blind | 0 | 3 | 3 | 1 | gpt-5.6-sol:fail, claude-fable-5:fail | a8a1c25e1bdc8a2991ad02324d3214e105deb8ff |

Consolidation = max across reviewers per severity, no cross-reviewer finding
matching. Full reviewer findings preserved at
`/tmp/opsx-cr-pipe-toggle/cr-sol.md` and `/tmp/opsx-cr-pipe-toggle/cr-fable.md`;
substance mirrored below.

## Findings (consolidated, round 1)

| # | Finding | Severity | Status |
|---|---|---|---|
| 1 | Attested worktree carried STALE judged inputs: delta spec still required `Event::Visible` while the sanctioned sync-query substitution (Scope Expansion #1) lived only on the integration checkout; cited AC `toggle-state-sync-query-verified` absent from any spec artifact in the attested tree (sol#1, fable#1 — Constitution II) | P1 | fixed (integration main merged into worktree; amended delta spec now in attested tree) |
| 2 | "Visible → hide" narrowed to "focused → hide"; visible-but-unfocused container state deviated from delta scenario letter (sol#2 P1 / fable#2 P2) | P1 | fixed (delta spec scenarios amended: visible-AND-focused hides; unfocused container state is shown — Scope Expansion #2) |
| 3 | R1/R2/R3 + regression evidence not recorded in the attested tree's review.md (sol#3) | P1 | fixed (Execution Notes landed integration-side and merged into worktree) |
| 4 | Cold-show retry decision logic hand-rolled in shim, not natively testable (fable#3, Constitution I advisory) | P2 | routed to follow-ups.md #1 |
| 5 | Regression S4 never exercised the stable-id→position conversion for a DRIFTED invoking tab (fable#4) | P2 | fixed (S5 added) — and S5 exposed a NEW upstream zellij defect: `break_multiple_panes_to_tab_with_index` final `get_indexed_tab_mut` is ID-keyed while its existence check and `go_to_tab` are position-based, so under drift the extracted pane is DROPPED (plugin instance killed; zellij log `screen.rs:4336 Could not find tab with index`). Cross-tab relocation under drift is currently impossible via the plugin API → decision-audit landing, awaiting owner ruling |
| 6 | Needless `pipe_message.payload.clone()` (fable#5) | P3 | fixed |

## Applied fixes

- Integration commit edfe8c4 (judged inputs: spec scenarios, Execution Notes,
  Scope Expansion #2, follow-ups.md) merged into worktree.
- Worktree commit 9c53620: post-relocation focus re-assert (S5-pre evidence:
  an unfocused relocated menu neither receives keys nor hides on next
  toggle), payload clone removed, regression S5 added, dump-layout EPIPE
  parse hardening.

## Residual risks

- See reviewer files; dominant open item is the S5-discovered upstream
  pane-loss defect (finding #5) — blocks the drift-case relocation branch and
  is the subject of the decision-audit landing.

## Verdict rationale

Round 1 consolidated fail (P1=3). All three P1s addressed; round 2 blocked on
the owner ruling for the drift-case relocation mechanism (finding #5) — the
frozen intent's "menu on the invoking tab regardless of id/position drift"
outcome is unachievable with any zellij 0.44.3 plugin-API primitive.
