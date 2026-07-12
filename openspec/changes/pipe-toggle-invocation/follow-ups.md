# Follow-ups

**Change:** pipe-toggle-invocation
**Created:** 2026-07-11 (first out-of-scope routing)

## Queue

| # | Finding | Severity | Origin (review type, round) | Routing reason | Status |
|---|---|---|---|---|---|
| 1 | `resolve_pending_cold_show` (harpoon-plugin/src/main.rs) hand-rolls the cold-show retry decision (budget policy + suppressed→relocate-vs-show choice) in the shim instead of routing fresh ground truth through a core decision fn — not natively testable (Constitution I advisory) | P2 | code-review round 1 (claude-fable-5 #3) | Advisory quality improvement; the shipped behavior is evidence-bound and regression-covered — not required for the frozen intent's outcomes | open |
| 2 | Regression/probe scripts seed the global zellij `permissions.kdl` with trap-based backup/restore; a crash between mutation and restore could leave a stray grant (pre-existing harness pattern from `cli-pipe-permission-regression.sh`) | P3 | code-review round 1 (claude-fable-5, residual risk) | Pre-existing harness-wide pattern, not introduced by this change; hardening it spans all three scenario scripts | open |

## Waivers

<!-- One entry per user-waived P0/P1 (decision-audit landing). -->

## Promotion

<!-- Filled at archive when the queue is non-empty. -->
