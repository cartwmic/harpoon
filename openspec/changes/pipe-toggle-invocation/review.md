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
- 2026-07-11 20:35 — Task 4.1+4.2 regression `scripts/toggle-pipe-regression.sh`
  6/6 PASS (worktree commit bf0453e; scenarios: cold-spawn show on invoking
  tab, visible-focused toggle hides, same-tab re-invoke after Esc-close,
  cross-tab invoke under forced tab-id/position drift lands menu+view on
  invoking tab). Native suite 244 green; wasm32-wasip1 clean.
  Implementation discoveries en route: keybind `MessagePlugin` WITHOUT
  `floating true` cold-spawns a TILED split (README warns); zellij's "About
  Zellij" tip pane (Plugin id 3 on fresh sessions) holds focus at first
  invoke — exposed the need for focused-pane identity in ground truth;
  cold-spawn pipe precedes host-side pane registration, fixed via
  `ToggleAction::ColdShow` + bounded `set_timeout(0.2)`×25 retry
  (suppressed panes receive no state events, so the retry rides
  `Event::Timer`).
- 2026-07-11 20:35 — R1 (cross-tab show-then-relocate flicker) evidence:
  the two host calls execute within a single pipe-handler invocation;
  scripted screen captures at settle time show only the final state (menu on
  invoking tab) — no artifact observable in captures. Live human
  observation deferred to runtime activation step 4 (README) with the
  frozen escalation trigger (worse than brief single-frame → report)
  restated there.
- 2026-07-11 22:10 — OWNER RULING (decision-audit landing): "fix" — user
  ruled the cross-tab mechanism should OPEN A NEW PANE ON THE INVOKING TAB
  ("why can't it just open a new pane in the tab I'm in?"). Verified
  feasible: `open_plugin_pane_floating` (zellij-tile 0.44.3 shim:1006) routes
  through `Action::NewFloatingPluginPane` — a new-pane action that never
  touches the broken focus/move paths and does not dedupe instances; own URL
  available fresh via `get_pane_info(own).plugin_url`. Mechanism: cross-tab
  show = spawn fresh instance floating+focused on the invoking tab, then old
  instance `close_self()`; same-tab show stays warm (`show_self`). Relocation
  via `break_panes_to_tab_with_index` ABANDONED (upstream defect #3,
  pane-loss). loop_hold cleared per this ruling; round budget extension
  granted implicitly (rounds continue, ledger retains round 1).
- 2026-07-11 21:45 — Regression S5 (drifted invoking tab, added per fable#4)
  exposed upstream zellij defect #3: `break_multiple_panes_to_tab_with_index`
  existence-check + go_to_tab are POSITION-based but the final
  `get_indexed_tab_mut(tab_index)` is `tabs.get_mut(&tab_index)` — STABLE-ID
  keyed — so when the target position has no same-numbered tab id, the
  extracted pane is silently DROPPED and the plugin instance dies (zellij
  log `screen.rs:4336 Could not find tab with index: 1`; observed "Bye from
  plugin"). Verified against v0.44.3 source. Consequence: drift-safe pane
  relocation is impossible in the 0.44.3 plugin API (audited all pane/tab
  movers: break_* is the only pane-to-tab primitive; FloatMultiplePanes is
  in-tab; MessageToPlugin cannot self-respawn a same-alias instance).
  Frozen-intent conflict → loop_hold set; decision-audit presented to owner.
  S5 left failing by design — it correctly detects the defect; its expected
  assertion depends on the ruling.
- 2026-07-12 07:40 — Respawn mechanism implemented (worktree 586c436):
  core `ToggleAction::Respawn` + `parked_on_focused_tab` ground truth
  (load-time parked-tab record, stale-safe — tab ids never reused);
  shim `open_plugin_pane_floating(own_url from get_pane_info, verbatim
  load config)` + `close_self`, safe degradation to `show_self` on spawn
  failure. Regression 9/9 incl. S4 (cross-tab respawn), S5 (respawn under
  drift), S6 (respawned instance still keybind-addressable — identity
  preserved). Native 244 green, wasm clean. NEW findings en route:
  (1) permission-DENIED response-decoding host calls PANIC the plugin (shim
  unwraps an empty response) — all such queries now gated behind
  `PermissionRequestResult(Granted)`; load()-time sync queries forbidden;
  (2) respawn requires `PermissionType::OpenTerminalsOrPlugins` — added to
  request_permission (Host Call Permission Completeness AC) + scripts +
  README runtime activation (visible-pane regrant step);
  (3) harness hazard: a STALE permissions.kdl entry for the same wasm path
  made zellij show an unanswerable interactive prompt — seed logic now
  rewrites (never skips) the entry.
- 2026-07-11 21:20 — Code-review round 1 (blind, 2 models) consolidated:
  P0=0 P1=3 P2=3 P3=1, both verdicts fail. Dispatch adapter reported both
  child runs as failed (bash exit 1) yet BOTH findings files were complete
  with valid attestations (HEAD a8a1c25e, worktree path) — counted per
  findings-file-sole-verdict; incident noted. Dominant P1 root: judged
  inputs (amended spec delta, Scope Expansions) were committed
  integration-side AFTER the worktree branched, so the attested tree
  carried stale copies — remedied by merging integration main into the
  worktree branch before round 2.

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
- 2026-07-11 — Cross-tab relocation MECHANISM substituted (owner-ruled at
  the decision-audit landing): frozen intent prescribed un-suppress via
  `show_self(true)` then `break_panes_to_tab_with_index`; regression S5
  proved the break host call DESTROYS the pane under tab-id/position drift
  (upstream defect #3 — position-based existence check + id-keyed
  `get_indexed_tab_mut`), and no drift-safe pane-to-tab mover exists in the
  0.44.3 plugin API. Substituted: respawn-on-invoking-tab —
  `open_plugin_pane_floating(own_url, own_config)` (new-pane action, never
  the broken paths) then `close_self()` on the old instance; same-tab stays
  warm. Frozen user-observable outcome (menu on invoking tab, view stays,
  drift-immune) now FULLY met — stronger than the original mechanism, which
  hopped the view through the parked tab. Cost: cold load (~0.1s) on
  cross-tab invokes only. intent.md untouched.
- 2026-07-11 — "Visible → hide" branch REFINED to "visible AND focused →
  hide; unfocused container state → bring to user": synchronous queries
  cannot distinguish a pipe-cold-spawned pane (parked floating, unfocused,
  invisible) from a user-visible unfocused pane (`get_pane_info` hardcodes
  `is_focused=false`; no layer-visibility query exists), and hiding the
  parked pane made the first invocation a visible no-op (regression run
  2026-07-11). In zellij's focus semantics the visible-but-unfocused
  floating state is effectively unreachable (showing focuses; focusing
  elsewhere hides the layer), and the degradation is benign (refocus, next
  toggle hides). Required to meet the frozen intent's user-observable
  outcome (invoke SHALL present the menu — branch 4/cold-spawn). Intent
  MEANING (toggle closes the open menu) unchanged; intent.md untouched.
  Delta spec scenarios amended accordingly (round-1 findings sol#2/fable#2).

## Fidelity Round Ledger

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|
