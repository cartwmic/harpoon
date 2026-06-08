## Why

The current harpoon-zellij UX is purely command-keyed: every printable char triggers a single action (`a`/`A`/`d`/`j`/`k`/etc.). With more than a handful of pinned panes the list is hard to scan, and there's no way to type-to-narrow the way users expect from `fzf`, Telescope, or the original Neovim harpoon. There's also no quick numeric jump and no way to reorder pinned panes once they're added — adding/removing is the only way to influence ordering. This change introduces three first-class, mutually-exclusive interaction modes (filter / command / jump), a configurable default, and a reorder operation, so harpoon can flex between fast type-to-find, deliberate command operations, and risk-free hotkey jumps.

## What Changes

- **BREAKING**: Reorganize key handling into three explicit modes — `command` (today's behavior), `filter` (typing narrows the list), and `jump` (read-only digit/letter hotkey jumps). Existing keybindings remain available *inside command mode*; users opening into a non-command default see different behavior on bare keys.
- Add a configurable `default_mode` (`command` | `filter` | `jump`). Plugin starts every session in this mode and resets to it on close.
- Add a filter mode with fuzzy matching, live match-highlighting in the rendered list, query rendered in place of the header, and `selected` snapping to the top match as the query changes.
- Add a jump mode where bare `1-9` then `a-z` jumps to slot N (positions 0..34 in the underlying pane list) and closes harpoon. All other keys are inert with respect to harpoon state (focusing the target pane is the operation's purpose).
- Make **digit-only** slot jumps (`1-9`) also available in command mode without requiring users to enter jump mode first. Letter slots (`a-z`) are reachable only via jump mode, because letters `a`/`c`/`d`/`j`/`k`/`l` are already bound to commands and cannot serve dual roles.
- Add reorder operations in command mode: `K` shifts the selected pane up, `J` shifts it down (saturating at boundaries). The pane list order *is* the slot order, so reordering directly remaps jump slots.
- Add slot prefixes (`1`, `2`, ..., `9`, `a`, ..., `z`) to every rendered row in `command` and `jump` modes; suppress prefixes in `filter` mode (filtered rows are reordered by score and the prefix character would be inert as a key). Add a mode badge `[N]`/`[F]`/`[J]` and per-mode color accent on the header in every mode.
- Add a vim-style Esc state machine: filter-with-non-empty-query Esc clears the query; filter-with-empty-query Esc returns to command; jump Esc returns to command; command Esc closes.
- Add config surface read from the plugin's `BTreeMap<String, String>` userspace config: `default_mode`, `matcher` (`fuzzy` | `substring`), `show_slots` (`true` | `false`).
- Add a fuzzy matcher (preferring `nucleo-matcher` if it builds for `wasm32-wasip1`, otherwise an in-tree subsequence-with-word-boundary-bonus matcher).

## Capabilities

### New Capabilities
- `mode-state-machine`: The three-mode model, transitions, default-mode config, and Esc semantics.
- `filter-mode`: Query input lifecycle, fuzzy matching, filtered selection semantics, match highlighting, and query rendering.
- `jump-mode`: Slot mapping (`1-9` → 0..8, `a-z` → 9..34), read-only key handling, and jump-then-close behavior. Also covers slot jumps available from command mode.
- `reorder`: `K`/`J` shift operations on the pane list, persistence interaction, selection follows the moved pane, saturation at boundaries.
- `plugin-config`: Reading and validating the plugin's userspace config (`default_mode`, `matcher`, `show_slots`).

### Modified Capabilities

(None — there are no pre-existing OpenSpec capabilities in this repo; this is the first change.)

## Impact

- **Code**: Crate restructured into a workspace with `harpoon-core` (lib, native-testable, no `zellij-tile` dep) holding the pure logic (modes, dispatch, matcher, slot mapping, reconciliation) and `harpoon-plugin` (cdylib, wasm32-wasip1) holding the `ZellijPlugin` impl, render layer, and persistence I/O. Per-mode key dispatch returns `Vec<Effect>` from pure handlers; the plugin shim applies effects in order. Render path adds slot prefixes (command/jump only), mode badge, query line, and char-indexed match highlighting (`color_indices` for fuzzy, `color_range` for substring). `hide_self` paths consolidated through a `close()` helper that resets `mode`/`query`/`selected`.
- **Data model**: `State` gains `mode: Mode`, `default_mode: Mode`, `query: String`, `config: Config`. `State.panes` becomes `Vec<Option<Pane>>` (sparse during the partial-restore window; compacted to dense on first user mutation via `freeze_on_user_mutation`). `Persistence` gains a first-class `pane_id_to_bookmark_idx: HashMap<u32, usize>` map (not persisted; rebuilt on load) for unambiguous pane↔bookmark identity tracking. `selected: usize` semantics shift to "index into filtered view" while in filter mode; recomputed `filtered_indices()` per render.
- **Persistence schema CHANGES (v1 → v2)**: top-level JSON wrapped in an envelope `{version: 2, bookmarks: [...]}`, and `PaneBookmark` gains an `index: Option<u16>` field recording the saved Vec position (`Some(i)` = saved-position placement on next reload; `None` = append-on-resolve, used for post-freeze entries). This is required because `match_pending_bookmarks` resolves bookmarks across multiple `update_panes` cycles (only currently-visible bookmarks resolve in any given round); without a saved index, late-resolving bookmarks would be appended to the end of `State.panes` rather than placed at their saved position, silently breaking the headline reorder-survives-reload guarantee. v1 (bare-array, no `index` field) bookmark files are read by assigning indices in array order; the next save writes v2. The existing `sort_panes()` helper is also removed from the `a`/`A` and bookmark-resolution paths so manual order remains canonical.
- **Dependencies**: Likely add `nucleo-matcher` (pure Rust, expected to build for `wasm32-wasip1`). If the spike fails, no new dep — in-tree matcher instead.
- **Config**: New plugin config keys consumed in `ZellijPlugin::load`. Backward compatible — all keys default safely (`default_mode = "command"` preserves today's bare-key UX).
- **Documentation**: `README.md` updated with the new keymap, mode model, and config example.
- **Users**: This is a personal fork; only one user (`cartwmic`) is affected. Defaulting `default_mode` to `command` keeps muscle memory intact for the existing add/delete/nav keys.
