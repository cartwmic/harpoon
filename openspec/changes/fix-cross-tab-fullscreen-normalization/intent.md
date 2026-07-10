# Intent: fix-cross-tab-fullscreen-normalization

Scale: M (full_rigor: false)
Frozen: 2026-07-09 (explore session; spikes 2026-07-09: cold-start cache + persistence/quirk)

## Intent

A `jump_pane` pipe (ntfy-driven) or keybind jump must deterministically land the
target pane focused AND fullscreen, in all four quadrants: plain/stacked
fullscreen layout × same-tab/cross-tab — including a cold-start plugin instance
whose state cache is empty (spike-proven: `manifest_cached=false
tab_info_cached=false` at pipe delivery, ~55ms after plugin load, zellij
0.44.3). Achieve this by upgrading `zellij-tile` 0.42.2 → 0.44.3 (matching the
installed runtime) and replacing predictive/cached fullscreen normalization in
`jump_focus_fullscreen` (harpoon-plugin/src/main.rs) with a synchronous
post-focus ground-truth query: focus target, query actual tab fullscreen state,
toggle only when provably tiled. No decision may depend on cached
`PaneInfo.is_fullscreen`, cached `TabInfo`, or pre-focus prediction.

Additionally, restore plugin-instance persistence: revert commit `d6a2039`
(close_self-on-close workaround) back to `hide_self()`. The zellij mis-focus
quirk that motivated it did not reproduce on zellij 0.44.3 (10/10
hide/relaunch cycles under the original trigger condition — fullscreen
terminal pane — with correct focus and exactly one plugin load). This
eliminates the ~47–92ms cold-start cost on every invocation and rejoins the
specs, which still mandate `hide_self` (pre-existing code/spec drift from
`d6a2039`).

## Constraints

- Build target `wasm32-wasip1`; deploy/hot-reload path unchanged
  (`~/.config/zellij/plugins/harpoon.wasm`).
- SDK bump scope: known signature change `focus_terminal_pane(id, true, false)`
  (3rd bool arg); Cargo.lock transitive churn accepted and reviewed as
  dependency-only noise. Zellij 0.44.3 becomes the supported runtime floor.
- Correctness MUST NOT depend on cached `PaneInfo`/`TabInfo` state or
  pre-focus prediction at pipe() time (2026-07-09 cold-start spike: both
  caches are `None` when a pipe-spawned instance receives the CLI
  `PipeMessage`; the pipe arrives BEFORE the first `TabUpdate`/`PaneUpdate`).
- Canonical close path is `hide_self()`; the effect ordering
  `[Effect::Close, Effect::FocusPane(id)]` (hide first, focus second) is
  preserved per the mode-state-machine spec.
- Broadcast pipes (no `--plugin` target) MUST NOT be used or recommended for
  `jump_pane`: zellij delivers them to all running plugin instances, and two
  live harpoon instances would each run normalization (double-toggle
  cancellation hazard). Plugin identity is (location, configuration); a
  configless pipe reaches a different instance than the configured keybind
  instance (spike-verified).
- Mandatory regression scenarios, driven by a committed repeatable spike
  script (tmux-hosted isolated zellij session, the 2026-07-09 harness),
  evidence recorded in tasks.md:
  1. cold-start pipe into fullscreen tab, target = hidden pane;
  2. cold-start pipe into fullscreen tab, target = the fullscreened pane
     itself;
  3. warm-cache cross-tab jump (the originally reported failure);
  4. hide → relaunch cycle with a fullscreen terminal pane (mis-focus quirk
     detector), plugin must re-show with focus and without a new load().
- Spec deltas required:
  - `pane-pipe-api` "Jump To Pane By Id": current text claims correctness in
    plain and stacked fullscreen layouts that the implementation does not
    deliver; update scenarios to cover cross-tab and cold-start.
  - `mode-state-machine` / `filter-mode` / `plugin-config`: code rejoins the
    existing `hide_self` mandate (no behavioral spec change; note the 0.44.3
    runtime floor and the persistence rationale where relevant).

## Invariants honored

- `openspec/opsx-gates.yaml`: `openspec validate --changes --strict`,
  `cargo build --release -p harpoon --target wasm32-wasip1`, and
  `cargo test -p harpoon-core` all green.
- `jump-mode` spec: "Jump fires and closes" and "Jump mode is read-only with
  respect to harpoon state" remain honored — normalization changes focus and
  fullscreen only, never slot state; hide satisfies the close-path wording,
  which already specifies `hide_self`.
- `plugin-config` spec: config read only in `load()`; a persistent instance
  does not re-read config (spec already describes instance survival across
  `hide_self`).
- `pane-pipe-api` "Pane Id String Parsing" behavior unchanged
  (`terminal_N` and bare `N` forms).

## Non-goals

- No changes to the notification chain (ntfy extension, termux-app,
  zellij-jump script, chezmoi). Adding matching `--plugin-configuration` to
  `~/bin/zellij-jump` so ntfy pipes reach the warm keybind instance is a
  latency optimization, NOT required for correctness (the configless twin
  instance is itself reused after its first pipe) — documented follow-up in
  the chezmoi repo, out of scope here.
- No event-driven pending-jump state machine (ttl/abort/event lifecycle) —
  rejected design; superseded by the synchronous post-focus query. Not an
  authorized fallback: if the synchronous query cannot be made correct, halt
  and surface to the owner rather than substituting a predictive design.
- No changes to slot management, filter mode, reorder, or persistence-to-disk
  behavior.
- No zellij runtime upgrade — 0.44.3 already installed; this aligns the SDK
  to it.
