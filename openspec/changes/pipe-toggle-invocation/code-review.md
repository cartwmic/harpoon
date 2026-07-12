# Code Review

**Change:** pipe-toggle-invocation
**Verdict:** pass
**review_mode:** blind
**reviewer-provenance:** openai-codex/gpt-5.6-sol, claude-bridge/claude-fable-5
**Diff Base SHA:** 402ac1e2024982a72615203a11bfb3d5ff42311d
**Reviewed Range:** 402ac1e2024982a72615203a11bfb3d5ff42311d..fff2f3f5bb1620d05c2713367b98ecebc0af6b1a
**Attested HEAD:** fff2f3f5bb1620d05c2713367b98ecebc0af6b1a
**Baseline:** intent.md + proposal + specs + design + plan + tasks status
**Generated:** 2026-07-11

## Verdict contract

Baseline-bounded review: fail only for a frozen-baseline violation or an
objective correctness/security defect. P0/P1 findings gate; P2/P3 do not.

## Round tracker

| Round | Mode | P0 | P1 | P2 | P3 | Reviewer verdicts | Reviewed HEAD |
|---|---|---|---|---|---|---|---|
| 1 | blind | 0 | 3 | 3 | 1 | gpt-5.6-sol:fail, claude-fable-5:fail | a8a1c25e1bdc8a2991ad02324d3214e105deb8ff |
| 2 | blind | 0 | 1 | 2 | 5 | gpt-5.6-sol:fail, claude-fable-5:pass | fd719aa551bce4d7a0feb1606a370ab583849aca |
| 3 | blind | 0 | 0 | 2 | 5 | gpt-5.6-sol:pass, claude-fable-5:pass | fff2f3f5bb1620d05c2713367b98ecebc0af6b1a |

Consolidation = max across reviewers per severity, no cross-reviewer finding
matching. Full reviewer findings preserved at
`/tmp/opsx-cr-pipe-toggle/cr-sol.md`, `cr-fable.md` (round 1), `r2-sol.md`,
`r2-fable.md` (round 2); substance mirrored below.

Round 2 (split verdict: sol fail P1=1, fable pass): consolidated P1 = the
parked-tab record was a grant-time FOCUSED-TAB PROXY, poisonable by a cold
`jump_pane` spawn (parks on A, focuses B) — a later toggle would warm-show
on the wrong tab (sol#1; fable flagged the same vector as P2). FIXED at
worktree 90fd231: recording now happens ONLY under focused-pane identity
verification (post-show, pre-hide, grant-time identity check); unknown
record ⇒ safe Respawn. Regression S7 pins the jump-spawn poisoning
scenario (11/11). Advisory routing: pre-grant shim decision + cold-show
retry policy (fable r2#2, sol r2#2) extend follow-ups #1; spawn-failure
fallback semantics (sol r2#3) routed as follow-ups #3; artifact drift
(fable r2#3) fixed in plan/tasks; theoretical persistence race +
double-invoke window + silent budget exhaustion (fable r2#4-6) accepted as
residual risks.

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

## Round 3 (quiet round — sealed)

Both reviewers pass at fff2f3f5bb1620d05c2713367b98ecebc0af6b1a (P0+P1 = 0;
findings files r3-sol.md, r3-fable.md). Round-3 advisories routed, none
gating: pre-grant/cold-show shim decision residue + spawn-failure fallback
(P2, already follow-ups #3/#4, reviewers concur 'deferred');
fable P3s — pending_cold_show not disarmed by an intervening Hide inside the
cold window; CLI-sourced toggle Respawn closes before the CLI unblock lands
(keybind path unaffected); transient two-instance overlap window during
respawn (~100ms); jump_focus_fullscreen response-decoding queries lack the
new pre-grant panic guard (PRE-EXISTING at Diff Base) — routed to
follow-ups #5-#8.

## Verdict rationale

Round 1 consolidated fail (P1=3: stale judged inputs in attested tree,
focused-vs-visible semantics, unrecorded evidence) — all fixed. Round 2
split (sol fail P1=1: parked-tab focused-proxy poisoning via jump_pane cold
spawn) — fixed with identity-verified recording + regression S7. Round 3
quiet: both models pass, doneness satisfied (blind-single-judge,
gpt-5.6-sol). Verdict: pass ⇔ no open P0/P1.
