# Intent — ntfy-harpoon-jump (harpoon slice)

Part of the cross-repo `ntfy-harpoon-jump` feature: tapping an ntfy notification
on the phone jumps the live remote zellij session to the exact tab/pane the
alerting process runs in. This slice adds the two harpoon plugin primitives that
everything else consumes. Sibling slices live in `termux-app` (phone-side intent
+ foreground) and `chezmoi` (SSH ControlMaster + remote notify script).

## Intent

Add two CLI-pipe handlers to the harpoon zellij plugin so external callers can
(1) resolve a pane to its harpoon slot and (2) jump focus to a pane by id,
deterministically and without depending on keyboard focus/mode state.

- `slot_for_pane` — reverse lookup. Input: a pane id (`$ZELLIJ_PANE_ID`, form
  `terminal_N` or bare `N`). Output on the CLI pipe's STDOUT via `cli_pipe_output`:
  the 1-based harpoon slot holding that pane, or empty string if the pane is not
  harpooned. Used at notification SEND time on the remote to stamp the ntfy
  payload.
- `jump_pane` — focus by pane id. Input: a pane id. Effect: focus that terminal
  pane (and its tab), reusing the existing deterministic fullscreen-safe jump
  logic. Used at notification TAP time, driven over the SSH side-channel from the
  phone. Chosen over injecting a harpoon hotkey into the PTY because it is immune
  to focused-pane mode state (e.g. vim insert) and to slot reassignment between
  send and tap.

## Constraints

- Pane-id matching MUST reconcile the `$ZELLIJ_PANE_ID` string form
  (`terminal_N`) with harpoon's stored `PaneInfo.id` (`u32`, terminal panes).
- `jump_pane` MUST reuse the existing `jump_focus_fullscreen(id)` two-phase
  approach (commit 6e88511) so it behaves correctly in plain AND stacked
  fullscreen layouts; do not reintroduce the superseded `PaneInfo.is_fullscreen`
  tab-level heuristic.
- `slot_for_pane` MUST NOT mutate harpoon state (pure read + `cli_pipe_output`).
- Build target MUST be `wasm32-wasip1` (native fails: undefined `_host_run_plugin_command`).
- Pipe permission surface: handlers must work under the plugin's existing pipe
  perms; do not broaden permissions beyond what CLI-pipe reception requires.
- Existing harpoon behavior and the 223-test suite MUST stay green.

## Invariants honored

- harpoon build/deploy/reload workflow: `cargo build --release -p harpoon
  --target wasm32-wasip1` → copy to `~/.config/zellij/plugins/harpoon.wasm` →
  `zellij action start-or-reload-plugin`.
- Fullscreen ground-truth: authoritative signal is `TabInfo.is_fullscreen_active`;
  hidden panes ARE present in the tab manifest.

## Non-goals

- No phone/Android work (termux-app slice).
- No SSH config or notify-script work (chezmoi slice).
- No new keybindings; jump is driven externally by pane id, not by slot hotkey.
- No slot auto-assignment for un-harpooned panes (fallback handled by notify script).
