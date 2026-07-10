# Code Review

**Change:** fix-cross-tab-fullscreen-normalization
**Verdict:** pass
**review_mode:** adversarial-multimodel
**reviewer-provenance:** pi-subagents parallel dispatch — openai-codex/gpt-5.6-sol (designated doneness judge), claude-bridge/claude-fable-5
**Diff Base SHA:** 096734ea4c80c9985debb04cf740f26a92961201
**Reviewed Range:** 096734ea4c80c9985debb04cf740f26a92961201..1303ceef02d777350da68b19473b57fb8ba56bb3
**Attested HEAD:** 1303ceef02d777350da68b19473b57fb8ba56bb3
**Baseline:** intent.md + proposal + specs + plan + tasks status
**Generated:** 2026-07-09

## Round tracker

| Round | Mode | P0 | P1 | P2 | P3 | Reviewer verdicts | Reviewed HEAD |
|---|---|---|---|---|---|---|---|
| 1 | blind | 0 | 4 | 1 | 3 | gpt-5.6-sol:fail fable-5:fail | ca77e20fc2af067b8822fe0eefe3efc53a27c0ba |
| 2 | blind | 0 | 0 | 2 | 4 | gpt-5.6-sol:pass fable-5:pass | 1303ceef02d777350da68b19473b57fb8ba56bb3 |

## Findings

Round 2 consolidated (max across reviewers; verdict sources /tmp/cr-r2-sol.md,
/tmp/cr-r2-fable.md). All round-1 P1s were fixed at 1303cee and neither
round-2 reviewer re-raised them.

| # | Finding | Severity | Status |
|---|---|---|---|
| 1 | effect.rs ordering doc still says "all other effects are commutative" — new ToggleFullscreenPane is order-sensitive vs FocusPane (doc-only; no co-emission today) | P2 | open |
| 2 | Harness fullscreen assertion is theme/config-fragile ('││' glyph, inherits operator config); recorded runs valid (frames provably on) | P2 | open |
| 3 | Harness depends on GNU `timeout`, not in Requirements comment; fails loud, never false-pass | P3 | open |
| 4 | loads_of greps path as regex (dots as wildcards); use grep -cF | P3 | open |
| 5 | PERM_BAK mktemp file not removed after restore; PERM_FILE unguarded if zellij setup output format drifts | P3 | open |
| 6 | T3.2 evidence placement note: worktree review.md Execution Notes carry bootstrap line only (evidence is in tasks.md in-range + integration review.md) | P3 | open |

## Applied fixes

- Round-1 #1/#9 → 1303cee: core-owned effect plan `post_focus_fullscreen_plan(truth, id) -> Vec<Effect>` + `Effect::ToggleFullscreenPane(u32)`; shim maps effects only.
- Round-1 #2 → 1303cee: harness S2 provably cold via distinct-path wasm identity + new-load assertion.
- Round-1 #3 → 1303cee: loads_of() single normalized integer.
- Round-1 #4 → 1303cee: permission seeding creates permissions.kdl when absent; wholesale delete at cleanup.
- Round-1 #5 → 1303cee: per-scenario evidence recorded in tasks.md (T3.2).
- Round-1 #7 → 1303cee: T5.1 wording corrected (two behavioral AC literals; docs AC README-grep-verified).
- Round-1 #8 → 1303cee: apply_effects doc comment re-attached.

## Residual risks

- Open P2/P3 warnings above (doc wording, harness robustness) — advisory,
  routed per finding to follow-ups.md where out-of-scope.
- Focus→query ordering is empirically synchronous (held in spike + both 8/8
  harness runs); a server-side reorder degrades safely to Unknown → no toggle
  (never a wrong-direction toggle).
- Pipe-while-UI-visible and floating-target cases: spec-sanctioned
  Unknown/no-toggle carve-out; routed to follow-ups.md.

## Verdict rationale

Round 2 quiet: both blind reviewers pass with 0 P0/P1 (consolidated max).
Both independently re-verified the mechanical gates read-only (strict
validate, wasm build, 238 core tests) and confirmed the diff touches no gate
manifest. Implementation judged baseline-faithful across Constitution I/IV/V,
the delta ACs, and the frozen intent's mandatory regression scenarios (8/8
PASS evidence in tasks.md). Multi-model requirement satisfied
(adversarial-multimodel, two distinct models, identical attested HEAD).
