## Why

The `ntfy-harpoon-jump` feature lets a phone ntfy-notification tap jump the live
remote zellij session to the exact pane the alerting process runs in. That jump
must be driven externally (over an SSH side-channel) and must be immune to
keyboard focus/mode state and to harpoon slot reassignment. harpoon owns the
pane↔slot map but currently exposes no external interface, so this slice adds the
two CLI-pipe primitives every sibling slice (`termux-app`, `chezmoi`) consumes.

## What Changes

- Add a `pipe()` handler to the harpoon plugin that receives zellij CLI pipe
  messages and answers two named requests.
- `slot_for_pane`: reverse lookup — given a pane id, return the 1-based harpoon
  slot via `cli_pipe_output`, or empty string if the pane is not harpooned. Pure
  read; MUST NOT mutate harpoon state.
- `jump_pane`: focus a pane by id, reusing the existing deterministic
  fullscreen-safe `jump_focus_fullscreen` path (commit 6e88511).
- Reconcile the `$ZELLIJ_PANE_ID` string form (`terminal_N` / bare `N`) with
  harpoon's stored `PaneInfo.id` (`u32`).

## Capabilities

### New Capabilities
- `pane-pipe-api`: external CLI-pipe interface exposing pane→slot reverse lookup
  and jump-by-pane-id against the live harpoon store.

### Modified Capabilities
<!-- none: additive interface; existing mode/jump/reorder behavior unchanged -->

## Impact

- **Affected files:**
  - `harpoon-plugin/src/main.rs` — add `pipe()` trait method + pane-id parsing;
    reuse existing `jump_focus_fullscreen`.
  - `harpoon-core/src/*` — a pure `slot_for_pane` lookup + pane-id-string parser
    (testable without the plugin/wasm host), consumed by the plugin shim.
  - tests under `harpoon-core` for the pure lookup + parser.
- **Build/permissions:** target stays `wasm32-wasip1`; handlers ride the plugin's
  existing pipe reception — no broadened permission surface.
- **Affects which projects:** enabling primitive for cross-repo `ntfy-harpoon-jump`
  (sibling slices in `termux-app` and `chezmoi` consume these pipes).
