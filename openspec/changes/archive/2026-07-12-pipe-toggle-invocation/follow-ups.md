# Follow-ups

**Change:** pipe-toggle-invocation
**Created:** 2026-07-11 (first out-of-scope routing)

## Queue

| # | Finding | Severity | Origin (review type, round) | Routing reason | Status |
|---|---|---|---|---|---|
| 1 | `resolve_pending_cold_show` (harpoon-plugin/src/main.rs) hand-rolls the cold-show retry decision (budget policy + suppressed→relocate-vs-show choice) in the shim instead of routing fresh ground truth through a core decision fn — not natively testable (Constitution I advisory) | P2 | code-review round 1 (claude-fable-5 #3) | Advisory quality improvement; the shipped behavior is evidence-bound and regression-covered — not required for the frozen intent's outcomes | open |
| 2 | Regression/probe scripts seed the global zellij `permissions.kdl` with trap-based backup/restore; a crash between mutation and restore could leave a stray grant (pre-existing harness pattern from `cli-pipe-permission-regression.sh`) | P3 | code-review round 1 (claude-fable-5, residual risk) + round 2 (sol #4) | Pre-existing harness-wide pattern, not introduced by this change; hardening it spans all three scenario scripts | open |
| 3 | Cross-tab respawn spawn-failure fallback calls `show_self(true)` on the old pane — knowingly navigates to the parked tab when `open_plugin_pane_floating` returns None; alternative: retain view + surface failure | P2 | code-review round 2 (gpt-5.6-sol #3) | Baseline does not define host-action failure semantics; fallback favors "something visible" over silent drop — semantics choice for a successor change | open |
| 4 | Pre-grant toggle branch + cold-show retry policy decided in shim rather than routed through core `toggle_plan` (extends #1) | P2/P3 | code-review round 2 (fable #2, sol #2) | Constitution I advisory; identical outcomes reachable via core all-None → ColdShow; native tests cover core, scenarios cover shim | open |
| 5 | `pending_cold_show` not disarmed by an intervening Hide inside the cold window — a Timer resolve can re-show an explicitly hidden menu (clear the flag in the Hide branch) | P3 | code-review round 3 (claude-fable-5 #2) | Narrow human-scale race; benign (next toggle hides); one-line successor fix | open |
| 6 | CLI-sourced `toggle` selecting Respawn calls `close_self()` before the CLI unblock lands — possible zombie pipe client on that path (keybind source unaffected; S6 covers keybind only) | P3 | code-review round 3 (claude-fable-5 #3) | CLI toggle is a documented-but-secondary surface; needs unblock-before-close ordering or scenario coverage | open |
| 7 | Respawn overlap window (~100ms, two instances with identical identity) — a targeted `jump_pane` arriving in-window could reach both (transient double-normalization hazard, domain invariant 6 analogue) | P3 | code-review round 3 (claude-fable-5 #4) | Practically negligible; human-paced ntfy taps | open |
| 8 | `jump_focus_fullscreen` response-decoding queries lack the pre-grant panic guard the toggle path gained (denied grant ⇒ plugin panic) — PRE-EXISTING at Diff Base, not introduced by this change | P3 | code-review round 3 (claude-fable-5 #6) | Outside this change's scope (jump path untouched); guard-discipline consistency fix for a successor | open |

## Waivers

<!-- One entry per user-waived P0/P1 (decision-audit landing). -->

## Promotion

<!-- Filled at archive when the queue is non-empty. -->
