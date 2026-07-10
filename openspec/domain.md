# harpoon Domain

**Version:** 1.0.0
**Last updated:** 2026-07-09

## Entities

- **Slot** — a hotkeyed bookmark position (1..n) mapping to a pane; slots are
  user-reorderable and reassignable at any time.
- **Pane** — a zellij terminal pane, identified by a stable `u32` id; exported
  to processes as `$ZELLIJ_PANE_ID` in the form `terminal_N`.
- **Tab** — a zellij tab; `tab.id` is stable for the tab's lifetime,
  `tab.position` is display order and changes as tabs move.
- **Pane manifest / TabInfo** — event-pushed snapshots
  (`PaneUpdate`/`TabUpdate`) that a plugin instance caches; they are absent
  (`None`) until the first event arrives.
- **Plugin instance** — a loaded wasm instance; zellij identity is the pair
  (plugin URL, configuration map). Instances with differing configuration are
  distinct plugins for pipe routing.
- **Pipe** — a named CLI→plugin message (`zellij pipe --name <n>`); harpoon
  listens on `jump_pane` and `slot_for_pane`.
- **Persistence file** — per-session bookmark state on disk, reloaded in
  `load()` keyed by session name.

## Invariants

1. Zellij fullscreen is a toggle; no absolute set-fullscreen host call exists.
   A toggle issued from an unknown fullscreen state is a defect.
2. Fullscreen is tab-level state: at most one fullscreen pane per tab
   (`fullscreen_is_active: Option<PaneId>`); when a tab is fullscreen,
   `toggle_pane_id_fullscreen` on ANY pane of that tab exits fullscreen
   (verified against zellij 0.44.3 server source).
3. Pane ids are stable and reassignment-immune; slot numbers are not.
   Cross-process references (notifications, pipes) MUST use pane ids, never
   slot numbers.
4. A pipe-spawned plugin instance receives the CLI `PipeMessage` BEFORE its
   first `TabUpdate`/`PaneUpdate`; its state caches are `None` at pipe time
   (spike-verified 2026-07-09, zellij 0.44.3).
5. Plugin instances with different configuration maps never share pipe
   messages: a configless pipe cannot reach the configured keybind instance.
6. Broadcast pipes (no `--plugin` target) deliver to ALL running plugin
   instances; with more than one harpoon instance loaded, broadcast
   `jump_pane` risks double normalization (toggle cancellation) and is
   forbidden.
7. Bookmark state survives plugin instance destruction only via the
   persistence file; in-memory state is per-instance.
8. Supported zellij runtime floor is 0.44.3; SDK (`zellij-tile`) tracks the
   installed runtime version.

## Units and conventions

- **Pane id strings**: accept both `terminal_N` and bare `N`; resolve to
  terminal pane `u32`.
- **Build**: Rust workspace; `harpoon-core` native-tested, `harpoon` binary
  compiled `wasm32-wasip1`.
- **Deploy**: `cp target/wasm32-wasip1/release/harpoon.wasm
  ~/.config/zellij/plugins/`; hot-reload via
  `zellij action start-or-reload-plugin`.
- **Naming**: capability specs kebab-case (`jump-mode`, `pane-pipe-api`).

## Out-of-scope domains

- The notification delivery chain (ntfy server/extension, termux-app fork,
  `~/bin/zellij-jump` SSH script, chezmoi dotfiles) — harpoon only owns the
  pipe surface those systems call.
- The zellij runtime itself — bugs/quirks in zellij are worked around or
  version-gated, never patched here.
- Shell/session management (mise, SSH ControlMaster) used by callers.

## See also

- Constitution: `openspec/constitution.md`
- Schema docs: `~/.local/share/openspec/schemas/opsx-superpowers/README.md`
