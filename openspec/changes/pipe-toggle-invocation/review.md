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
| Scale | M | Frozen intent.md recommends Scale M, `full_rigor: false` (single-capability, cross-file: keybind invocation replacement — pipe handler + Event::Visible wiring + cross-tab relocation + spec delta + regression scenarios) |
| full_rigor | false | Per intent recommendation — no cross-capability/breaking/migration content |
| Execution Mode | standard | |
| Verification Mode | retained-recommended | |
| Debug Mode | standard | |
| Review Status | not-requested | |
| Delegation Mode | single-agent | |
| Code Review Mode | derived (absent) | M ⇒ gating-required (derived fail-closed) |
| Loop Max Iterations | 40 | Authoring-time default for M |
| Validation Source Mode | required | Committed tmux-hosted regression scenario script(s) are the agent-independent validation source (precedent: scripts/fullscreen-regression.sh) |
| Doneness Mode | required | Template default retained; plain-M ⇒ doneness rides the code-review dispatch (designated reviewer) |
| Spec Level | spec-anchored | Default |
| Model Config | (unset) | Roles resolve via `opsx models`; session model fallback |

## Diff Base + Worktree locator

**Diff Base SHA:** 402ac1e2024982a72615203a11bfb3d5ff42311d
**Worktree Path:** /Volumes/Workshop/git/harpoon--opsx-pipe-toggle-invocation
**Integration Branch:** main

## Manual Adjustments

- Scale M adopted directly from frozen intent.md ("Recommended Scale: M,
  `full_rigor: false`") — no deviation; autonomous loop recorded this as an
  assumption rather than pausing to confirm.

## Execution Notes

<!-- Transient observations appended during apply. One-line entries when a
non-trivial decision is made mid-task. Durable knowledge → retrospective.md. -->

- 2026-07-11 19:25 — Tasks 1.1+1.2 probes 7/7 (`scripts/toggle-pipe-probe.sh`,
  worktree commit 6ca7fd5). R2 RESOLVED: keybind `MessagePlugin` pipe reaches
  the loaded (even suppressed) plugin, `source=Keybind`, zero permission
  denials. R3 RESOLVED: cached `TabUpdate`/`PaneUpdate` FREEZE while the pane
  is suppressed (probe: cached_active_tab=0/unsuppressed after a real hide +
  tab switch), while synchronous queries are fresh
  (`get_focused_pane_info`→tab id 1, `get_tab_info`→(pos 1, active),
  `get_pane_info(own)`→suppressed=true). Bonus finding: `Event::Visible` is
  emitted ONLY to tiled plugin panes (zellij tab/mod.rs `Tab::visible()`
  filters `tiled_panes.pane_ids()`) — floating harpoon NEVER receives it;
  probe observed zero deliveries. Also: `get_focused_pane_info()` returns the
  STABLE TAB ID (screen.rs `active_tab_ids`), not a position — convert via
  `get_tab_info(id).position` before any position-based host call; and
  tab-side `get_pane_info` hardcodes `is_focused=false` (never use it for
  focus decisions).

## Scope Expansions

<!-- Evidence-gated widenings (opsx-adversarial-review). One entry per widening;
surfaced at the decision-audit landing or gate-green. -->

- 2026-07-11 — Visibility-state MECHANISM substituted: frozen intent
  prescribed `Event::Visible` subscription with event-derived visibility;
  probe evidence (task 1.1/1.2, 7/7) shows zellij emits `Event::Visible`
  only to TILED plugin panes (floating harpoon never receives it) and event
  caches freeze while suppressed — the prescribed mechanism is structurally
  unavailable. Substituted synchronous host queries (`get_pane_info`,
  `get_focused_pane_info` + `get_tab_info`), which satisfy the same frozen
  invariant the constraint cited (Constitution IV: verified, never assumed)
  strictly better. Intent MEANING (verified visibility state; the
  user-observable outcome) unchanged; intent.md untouched. Spec delta
  requirement renamed accordingly (`toggle-state-sync-query-verified`).
  Evidence: Execution Notes 2026-07-11; `scripts/toggle-pipe-probe.sh`.

## Fidelity Round Ledger

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|
