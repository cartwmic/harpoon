<!-- authored: in-session -->
## Why

Notification-driven (`jump_pane` pipe) and keybind jumps into tabs that are
already fullscreen land the correct pane TILED instead of fullscreen. Root
cause violates Constitution IV (never act on unverified host state): the
current `jump_focus_fullscreen` predicts zellij's focus side effects from
cached `TabInfo`/`PaneInfo`, but a pipe-spawned instance's caches are `None`
at pipe delivery (Domain invariant 4) and zellij's focus-while-fullscreen side
effects diverge by layout — no prediction is correct in all quadrants. The
frozen baseline is `intent.md` (commit ad1963c).

## What Changes

- Bump `zellij-tile` 0.42.2 → 0.44.3 (matches installed runtime; 0.44.3
  becomes the supported floor; known signature change:
  `focus_terminal_pane(id, true, false)`).
- Rewrite fullscreen normalization in `jump_focus_fullscreen`
  (harpoon-plugin/src/main.rs): replace predictive/cached logic with a
  synchronous post-focus ground-truth query — focus target, query actual tab
  fullscreen state, toggle only when provably tiled. Deletes the
  cross-tab/`in_active` gate and the cached `PaneInfo.is_fullscreen` guard.
- Revert commit d6a2039: canonical close path returns from `close_self()` to
  `hide_self()` (mis-focus quirk dead on zellij 0.44.3, spike-verified
  2026-07-09 — 10/10 hide/relaunch cycles, 1 plugin load). Restores instance
  persistence, eliminates ~47–92ms cold start per invocation, and rejoins the
  specs that still mandate `hide_self` (pre-existing drift).
- Commit a repeatable regression spike script (tmux-hosted isolated zellij
  session) driving the four mandatory scenarios from intent.md; evidence
  recorded in tasks.md.
- No broadcast pipes for `jump_pane` anywhere in code or docs (Domain
  invariant 6).

## Capabilities

### New Capabilities

- (none)

### Modified Capabilities

- `pane-pipe-api`: "Jump To Pane By Id" — outcome guarantee restated over
  ground-truth normalization; scenarios extended to cross-tab and cold-start
  (empty cache) delivery; broadcast-pipe prohibition documented.
- `mode-state-machine`: close path re-anchored on `hide_self()` as the code
  rejoins the existing mandate; zellij 0.44.3 runtime floor noted.
- `plugin-config`: instance-persistence prose re-confirmed (config read only
  in `load()`, instance survives `hide_self`); 0.44.3 floor noted.
- `jump-mode`: jump close-path wording re-confirmed against `hide_self`
  (no behavioral requirement change; drift note only).

## Impact

**Affected files**

- `Cargo.toml`, `Cargo.lock` (workspace `zellij-tile` bump; transitive churn)
- `harpoon-plugin/src/main.rs` (`jump_focus_fullscreen`, `close_helper`,
  `focus_terminal_pane` call sites, stale design-constraint comments)
- `harpoon-core` (effect ordering tests if signatures ripple; no behavior
  change expected)
- `openspec/specs/{pane-pipe-api,mode-state-machine,plugin-config,jump-mode}/spec.md`
  (delta specs in this change)
- New: regression spike script under the repo (committed harness)

**Affects which projects**

- harpoon only. The chezmoi `~/bin/zellij-jump` `--plugin-configuration`
  warm-instance optimization is explicitly out of scope (intent Non-goals;
  documented follow-up in the chezmoi repo).
