# mode-state-machine Specification

## Purpose
TBD - created by archiving change add-filter-and-jump-modes. Update Purpose after archive.
## Requirements
### Requirement: Three mutually-exclusive interaction modes

The plugin SHALL implement exactly three interaction modes — `Command`, `Filter`, `Jump` — and SHALL be in exactly one mode at any time the plugin pane is open.

#### Scenario: Plugin always in exactly one mode while open
- **WHEN** the plugin pane is visible
- **THEN** the internal `State.mode` is set to one of `Command`, `Filter`, or `Jump`
- **AND** never to a combined or null value

#### Scenario: Mode badge reflects current mode
- **WHEN** the plugin renders in any mode
- **THEN** the rendered header line includes a 3-char badge that is `[N]` for `Command`, `[F]` for `Filter`, or `[J]` for `Jump`
- **AND** the badge text is rendered with a `Text::color_range` accent at level 2
- **AND** the badge text is the primary mode discriminator; the accent color is theme-driven (level 2) and is the same across modes — zellij-tile's API does not currently expose per-mode color at a fixed emphasis level. If a future zellij-tile API allows per-call colors, this scenario should be tightened to require distinct per-mode hues

### Requirement: Configurable default mode

The plugin SHALL read a `default_mode` config value from its userspace config map on `load`, accepting `command`, `filter`, or `jump`. Any other value SHALL fall back to `command`. The plugin SHALL initialize `State.mode` to this value and SHALL reset to it whenever the plugin closes.

#### Scenario: Default mode applied on first load
- **WHEN** the plugin is loaded with config `default_mode = "filter"`
- **THEN** `State.default_mode` is `Filter`
- **AND** `State.mode` is initialized to `Filter`

#### Scenario: Unknown default mode falls back
- **WHEN** the plugin is loaded with config `default_mode = "wibble"`
- **THEN** `State.default_mode` is `Command`
- **AND** `State.mode` is initialized to `Command`

#### Scenario: Default mode missing entirely
- **WHEN** the plugin is loaded with no `default_mode` config key
- **THEN** `State.default_mode` is `Command`

#### Scenario: Mode resets on close
- **WHEN** the user closes the plugin from any mode
- **THEN** `State.mode` is set to `State.default_mode`
- **AND** `State.query` is cleared

### Requirement: Vim-style Esc transitions

`Esc` SHALL transition between modes per the following rules: from `Filter` with non-empty query → clear query and stay in `Filter`; from `Filter` with empty query → `Command`; from `Jump` → `Command`; from `Command` → close.

#### Scenario: Esc clears non-empty query in filter mode
- **GIVEN** mode is `Filter` and query is `"ed"`
- **WHEN** the user presses `Esc`
- **THEN** query becomes `""`
- **AND** mode remains `Filter`

#### Scenario: Esc returns to command from empty filter
- **GIVEN** mode is `Filter` and query is `""`
- **WHEN** the user presses `Esc`
- **THEN** mode becomes `Command`

#### Scenario: Esc returns to command from jump
- **GIVEN** mode is `Jump`
- **WHEN** the user presses `Esc`
- **THEN** mode becomes `Command`

#### Scenario: Esc closes from command
- **GIVEN** mode is `Command`
- **WHEN** the user presses `Esc`
- **THEN** the plugin closes
- **AND** mode resets to `default_mode`
- **AND** query is cleared

### Requirement: Mode entry from command mode

`Command` mode SHALL provide explicit transitions: `/` enters `Filter`, `#` enters `Jump`. These transitions SHALL be available only from `Command` mode.

#### Scenario: Slash enters filter mode
- **GIVEN** mode is `Command`
- **WHEN** the user presses `/`
- **THEN** mode becomes `Filter`
- **AND** query is empty

#### Scenario: Hash enters jump mode
- **GIVEN** mode is `Command`
- **WHEN** the user presses `#`
- **THEN** mode becomes `Jump`

#### Scenario: Slash is a query character outside command mode
- **GIVEN** mode is `Filter`
- **WHEN** the user presses `/`
- **THEN** query gains the character `/`
- **AND** mode remains `Filter`

### Requirement: Close consolidation

All paths that hide the plugin (`Enter`/`l` focus, `c`, `Ctrl+c`, `Esc` from command, `a` add focused, slot-jump from any mode) SHALL flow through a single `close` helper that:

1. Calls `hide_self()` (in the application shim, after the handler returns).
2. Sets `State.mode = State.default_mode`.
3. Clears `State.query`.
4. Calls `reanchor_selected_to_focus(state, focused_idx)` to re-anchor `State.selected` so that the next open lands on a valid index without depending on a subsequent `PaneUpdate` event. (View-length clamp is the caller's responsibility on the next render path.)

**Note**: `A` (add all visible panes) does NOT close — see the next requirement.

When the close path is also a focus path (jumps from any mode, filter `Enter`, command `Enter`/`l`), handlers SHALL emit effects in the order `[Effect::Close, Effect::FocusPane(id)]` so that the shim invokes `hide_self()` first and `focus_terminal_pane(id, true)` second, matching the existing sequence in `src/main.rs`.

#### Scenario: Add focused pane resets mode and query
- **GIVEN** mode is `Command` and a pane is focused
- **WHEN** the user presses `a`
- **THEN** the focused pane is added to the list
- **AND** the plugin closes
- **AND** mode resets to `default_mode`
- **AND** query is cleared

#### Scenario: Reopen after close lands on valid selected index
- **GIVEN** the user closed the plugin from `Filter` mode with `selected = 5` in the filtered view
- **AND** the underlying `State.panes` has 3 entries
- **WHEN** the plugin is reopened
- **THEN** `State.selected` is in `[0, panes.len() - 1]`
- **AND** rendering does not panic on the first frame, even before any `PaneUpdate` arrives

#### Scenario: Effect order is Close then FocusPane
- **GIVEN** mode is `Command` and `panes[0]` exists
- **WHEN** the user presses `1` (digit jump)
- **THEN** the handler returns `[Effect::Close, Effect::FocusPane(panes[0].pane_info.id)]`
- **AND** the shim calls `hide_self()` before `focus_terminal_pane(id, true)`

### Requirement: A (add all) does not close the plugin

In `Command` mode, the `A` (add all visible panes) command SHALL append matching panes to `State.panes` and emit `[Effect::Save, Effect::Render]` only. It SHALL NOT emit `Effect::Close`. The plugin remains open after the add so the user can immediately reorder (`K`/`J`), filter (`/`), or jump (`#` then a slot key) the freshly added panes.

This intentionally diverges from `a` (add focused), which closes after add to match the today-typical "pin and fly back to work" muscle memory.

#### Scenario: A adds without closing
- **GIVEN** mode is `Command` and 3 visible panes exist that are not yet pinned
- **WHEN** the user presses `A`
- **THEN** all 3 panes are appended to `State.panes`
- **AND** the handler returns `[Effect::Save, Effect::Render]`
- **AND** the handler does NOT emit `Effect::Close`
- **AND** the plugin remains visible after the keystroke
- **AND** mode remains `Command`

#### Scenario: A then K reorders newly added pane without reopening plugin
- **GIVEN** mode is `Command` and `selected = 0` (head of the list)
- **WHEN** the user presses `A` (which appends N panes), then immediately presses `j` to navigate down to a newly added pane, then presses `K`
- **THEN** all keystrokes are processed by a single open-plugin session
- **AND** the reorder takes effect without re-opening the plugin

### Requirement: Ctrl+c closes from command mode

In `Command` mode, the `c` key SHALL accept ANY modifier set (including `Ctrl+c`) and close the plugin. This is an explicit carve-out from the otherwise-strict modifier gating on letter keys; today's existing fork code (`BareKey::Char('c')` matched without modifier checking) silently accepts `Ctrl+c` as close, and removing that behavior would silently break the user's muscle memory for closing the plugin via the universal cancel chord.

#### Scenario: Ctrl+c closes from command mode
- **GIVEN** mode is `Command`
- **WHEN** the user presses `Ctrl+c`
- **THEN** the handler returns `vec![Effect::Close]`
- **AND** the plugin closes

#### Scenario: Plain c closes from command mode
- **GIVEN** mode is `Command`
- **WHEN** the user presses `c` with no modifiers
- **THEN** the handler returns `vec![Effect::Close]`
- **AND** the plugin closes

#### Scenario: Other modified letters in command mode are no-ops
- **GIVEN** mode is `Command`
- **WHEN** the user presses `Ctrl+a` (which would otherwise be the add command)
- **THEN** the handler returns `vec![Effect::Noop]`
- **AND** no pane is added
- **AND** the plugin remains open

### Requirement: Command-mode j/k navigation wraps

In `Command` mode, `j` (down) and `k` (up) SHALL navigate `selected` with **wrapping** semantics, matching today's existing fork behavior:
- `j` when `selected == panes.len() - 1` SHALL wrap to `selected = 0`.
- `k` when `selected == 0` SHALL wrap to `selected = panes.len() - 1`.

This intentionally differs from `K`/`J` (reorder) which saturate at boundaries: nav is cheap to undo (one keystroke), reorder mutates persistent state and overshoot would feel destructive.

#### Scenario: j wraps from bottom to top
- **GIVEN** mode is `Command`, `panes.len() = 4`, `selected = 3`
- **WHEN** the user presses `j`
- **THEN** `selected` becomes `0`
- **AND** the handler returns `vec![Effect::Render]`

#### Scenario: k wraps from top to bottom
- **GIVEN** mode is `Command`, `panes.len() = 4`, `selected = 0`
- **WHEN** the user presses `k`
- **THEN** `selected` becomes `3`
- **AND** the handler returns `vec![Effect::Render]`

#### Scenario: j/k on empty list is a no-op
- **GIVEN** mode is `Command` and `panes.is_empty()`
- **WHEN** the user presses `j` or `k`
- **THEN** `selected` is unchanged
- **AND** the handler returns `vec![]` (no Render, no Save)

#### Scenario: j/k on single-element list is a no-op
- **GIVEN** mode is `Command` and `panes.len() == 1` and `selected == 0`
- **WHEN** the user presses `j` or `k`
- **THEN** `selected` remains `0`
- **AND** the handler returns `vec![]` (no observable change)

### Requirement: Every state-mutating key path triggers render

Every key path in any mode that mutates `State.mode`, `State.query`, `State.selected`, or `State.panes` SHALL cause the enclosing `update()` invocation to return `should_render = true`. Key paths that have no observable effect (no-ops, ignored keys in jump mode, Backspace on empty query) MAY return `should_render = false`.

#### Scenario: Mode transition triggers render
- **GIVEN** mode is `Command`
- **WHEN** the user presses `/` (which transitions to `Filter`)
- **THEN** `update()` returns `should_render = true`

#### Scenario: Query mutation triggers render
- **GIVEN** mode is `Filter` and query is `"e"`
- **WHEN** the user presses `d` (appending to query)
- **THEN** `update()` returns `should_render = true`

#### Scenario: Reorder triggers render
- **GIVEN** mode is `Command` and `selected > 0`
- **WHEN** the user presses `K`
- **THEN** `update()` returns `should_render = true`

#### Scenario: Ignored keys do not force render
- **GIVEN** mode is `Jump`
- **WHEN** the user presses an arrow key (ignored in jump mode)
- **THEN** `update()` may return `should_render = false`

