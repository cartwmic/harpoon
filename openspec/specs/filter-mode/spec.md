# filter-mode Specification

## Purpose
TBD - created by archiving change add-filter-and-jump-modes. Update Purpose after archive.
## Requirements
### Requirement: Query input lifecycle

In `Filter` mode, printable characters SHALL append to `State.query`, `Backspace` SHALL remove the last character, and the rendered list SHALL update to show only matching panes after every keystroke.

#### Scenario: Typing a character appends to the query
- **GIVEN** mode is `Filter` and query is `"ed"`
- **WHEN** the user presses `i`
- **THEN** query becomes `"edi"`
- **AND** the rendered list re-filters

#### Scenario: Backspace erases the last character
- **GIVEN** mode is `Filter` and query is `"edit"`
- **WHEN** the user presses `Backspace`
- **THEN** query becomes `"edi"`

#### Scenario: Backspace on empty query is a no-op
- **GIVEN** mode is `Filter` and query is `""`
- **WHEN** the user presses `Backspace`
- **THEN** query remains `""`
- **AND** mode remains `Filter`

### Requirement: Fuzzy matching with configurable algorithm

The matcher SHALL accept a `matcher` config value of `fuzzy` (default) or `substring`. The match input SHALL be the row's display string `"<tab_name> | <pane_title>"`, EXCLUDING any slot prefix. The match SHALL be case-insensitive (ASCII case-fold).

Matched panes SHALL be ordered by `(score DESC, original_panes_index ASC)`: highest score first, with the smaller original `State.panes` index sorting first as a stable tie-breaker. Tie-breaking on original index ensures that consecutive renders with the same query produce the same row order regardless of the matcher's internal iteration, so `selected = 0` lands on the same pane on repeated keystrokes.

**Placeholder exclusion**: filter mode operates on Live panes only. `State.panes` entries that are `None` (placeholders for unresolved bookmarks during the partial-restore window) SHALL NOT appear in the filtered view, regardless of query content. The matcher only iterates `state.panes.iter().enumerate().filter_map(|(i, opt)| opt.as_ref().map(|p| (i, p)))`.

#### Scenario: Fuzzy matcher is case-insensitive
- **GIVEN** the pane list contains a row with display string `"Work | Edit log"`
- **AND** mode is `Filter` and matcher is `fuzzy`
- **WHEN** the user types `"edit"`
- **THEN** the row matches
- **AND** appears in the filtered list

#### Scenario: Slot prefix is excluded from matching
- **GIVEN** the pane list has 3 rows with slot prefixes `1`, `2`, `3`
- **AND** mode is `Filter`
- **WHEN** the user types `"1"`
- **THEN** the result depends only on the rows' display strings
- **AND** is NOT trivially every row by virtue of the prefix

#### Scenario: Substring matcher is exact
- **GIVEN** matcher is `substring`
- **AND** the pane list has a row `"shell | edit log"`
- **WHEN** the user types `"el"`
- **THEN** that row matches (case-insensitive substring)
- **AND** a row `"build | cargo watch"` does NOT match

#### Scenario: Empty query shows all panes in original order
- **GIVEN** mode is `Filter` and query is `""`
- **WHEN** the plugin renders
- **THEN** every pane in `State.panes` is shown
- **AND** the order matches `State.panes` order (slot order)

#### Scenario: Equal-score matches are tie-broken by original index
- **GIVEN** mode is `Filter` and query is `"e"`
- **AND** `panes[0]` and `panes[3]` both have display strings that match with the SAME fuzzy score
- **WHEN** the plugin renders
- **THEN** `panes[0]` appears before `panes[3]` in the filtered view
- **AND** repeated keystrokes producing the same query produce the same order

### Requirement: Selected anchors to top match on query change

When the query changes, `selected` SHALL be set to `0` (the top-scoring filtered match). When the query is empty, `selected` SHALL fall back to the focused pane's index in the full list, matching pre-filter behavior.

#### Scenario: Selected jumps to top match on first keystroke
- **GIVEN** mode is `Filter` and query is `""`
- **AND** focused pane is at index 2 in the list
- **WHEN** the user types a character that produces a non-empty query
- **THEN** `selected` becomes `0` (top of filtered view)

#### Scenario: Selected returns to focused pane when query cleared
- **GIVEN** mode is `Filter` and query is `"ed"` with `selected = 0`
- **AND** focused pane is at index 2
- **WHEN** the user backspaces until query is `""`
- **THEN** `selected` returns to the index of the focused pane

#### Scenario: Query clear with no focused pane in tracked list
- **GIVEN** mode is `Filter` and query is `"ed"`
- **AND** `focused_pane` is `None` OR `focused_pane` is not present in `State.panes`
- **WHEN** the user backspaces until query is `""`
- **THEN** `selected` is set to `0` (clamped against current `panes` length)
- **AND** rendering does not panic

#### Scenario: Selected stays valid when filter empties
- **GIVEN** mode is `Filter` and query is `"ed"` matching 2 rows with `selected = 0`
- **WHEN** the user types another character making query `"edx"` matching 0 rows
- **THEN** `selected` is `0`
- **AND** rendering shows zero rows without crashing

### Requirement: Match highlighting via character indices

Each rendered filtered row SHALL color-highlight the characters that contributed to the match, using **character (Unicode scalar) indices**, not byte offsets. Slot prefix offset MUST also be expressed in character indices, but is unconditionally `0` in `Filter` mode because slot prefixes are suppressed there (see `jump-mode/spec.md`).

Matcher implementations SHALL return character indices into the haystack. The render layer SHALL dispatch on matcher type:
- For `fuzzy` matches (potentially non-contiguous indices): apply `Text::color_indices(level, char_indices)`. `color_indices` is the `zellij-tile` API designed for arbitrary index lists (see `ui_components/text.rs`).
- For `substring` matches (always contiguous): apply `Text::color_range(level, start..end)` where `start` and `end` are character indices.

Using `color_range` for fuzzy is incorrect because `color_range` accepts a single `RangeBounds<usize>` and cannot express non-contiguous index sets.

#### Scenario: Fuzzy matches highlight via color_indices (ASCII, non-contiguous)
- **GIVEN** mode is `Filter`, matcher is `fuzzy`, query is `"ed"`
- **AND** a matching row's display string is `"shell | edit log"`
- **WHEN** the plugin renders
- **THEN** the rendered text applies `Text::color_indices(level, vec![8, 9])` (character positions of `e` and `d`)
- **AND** does NOT call `color_range` for the fuzzy highlight

#### Scenario: Substring matches highlight via color_range (contiguous)
- **GIVEN** matcher is `substring` and query is `"edit"`
- **AND** a matching row's display string is `"shell | edit log"`
- **WHEN** the plugin renders
- **THEN** the rendered text applies a single `Text::color_range(level, 8..12)` covering character indices 8 through 11

#### Scenario: Multi-byte haystack highlights at correct character positions
- **GIVEN** matcher is `substring` and query is `"log"`
- **AND** a matching row's display string is `"📦 build | tail log"`
- **AND** character positions are: `📦`=0, ` `=1, `b`=2, `u`=3, `i`=4, `l`=5, `d`=6, ` `=7, `|`=8, ` `=9, `t`=10, `a`=11, `i`=12, `l`=13, ` `=14, `l`=15, `o`=16, `g`=17
- **WHEN** the plugin renders
- **THEN** the rendered text applies `color_range(level=1, 15..18)` covering character positions 15, 16, 17 (the trailing `l`, `o`, `g`)
- **AND** the highlight visually aligns with the matched characters (NOT shifted by the byte width of `📦`)

### Requirement: Query line rendering

When mode is `Filter` and query is non-empty, the header line SHALL be replaced with a query line of the form `"/<query>"` plus a `(<matches>/<total>)` count and the mode badge.

#### Scenario: Query line shown with non-empty query
- **GIVEN** mode is `Filter` and query is `"ed"`
- **AND** the matcher returns 2 matches out of 4 panes
- **WHEN** the plugin renders
- **THEN** the header line displays `"/ed"`, the count `"(2/4)"`, and the badge `[F]`
- **AND** the standard `==== N panes ====` header is NOT shown

#### Scenario: Standard header shown with empty query
- **GIVEN** mode is `Filter` and query is `""`
- **WHEN** the plugin renders
- **THEN** the header line displays the standard `==== N panes ====` header plus the badge `[F]`

### Requirement: Enter focuses filter selection and closes

In `Filter` mode, `Enter` SHALL focus the pane currently at `selected` in the filtered view (i.e. the top-scoring match when query is non-empty after `selected = 0` snap, or whatever pane the user navigated to via arrow keys), then close the plugin. The handler SHALL emit `[Effect::Close, Effect::FocusPane(id)]` in that order so that the application sequence is `hide_self()` followed by `focus_terminal_pane(id, true)`, matching the existing close-and-focus behavior in `src/main.rs`.

#### Scenario: Enter focuses selection
- **GIVEN** mode is `Filter`, query is `"ed"`, and the top match (selected) is the pane at `panes[2]`
- **WHEN** the user presses `Enter`
- **THEN** the handler emits `[Effect::Close, Effect::FocusPane(panes[2].pane_info.id)]`
- **AND** the shim calls `hide_self()` first, then `focus_terminal_pane(id, true)`
- **AND** mode resets to `default_mode`
- **AND** query is cleared

### Requirement: Esc clears query and re-anchors selected

In `Filter` mode, when the query is non-empty and the user presses `Esc`, the query SHALL be cleared AND `reconcile_selected(state, focused_idx)` SHALL be called so that `selected` is re-anchored to the focused pane (or `0` if focused pane is `None`). This makes Esc-clear and backspace-to-empty produce identical post-conditions; both reach the same empty-query state.

#### Scenario: Esc-clear matches backspace-clear behavior for selected
- **GIVEN** mode is `Filter`, query is `"ed"`, `selected = 0`, focused pane is at index 2
- **WHEN** the user presses `Esc`
- **THEN** query becomes `""`
- **AND** `selected` is set to `2` (focused pane index, via reconcile_selected)
- **AND** mode remains `Filter`

#### Scenario: Enter on empty filtered view is a no-op
- **GIVEN** mode is `Filter` and the filter produces zero matches
- **WHEN** the user presses `Enter`
- **THEN** no pane is focused
- **AND** the plugin remains open

### Requirement: Filter mode does not consume command keys

Printable characters that would be commands in `Command` mode (e.g. `a`, `A`, `d`, `j`, `k`, `K`, `J`, `c`, `l`, digits, letters) SHALL be treated as query input while in `Filter` mode, NOT as commands or slot triggers.

#### Scenario: Letter `a` filters instead of adding
- **GIVEN** mode is `Filter`
- **WHEN** the user presses `a` (no modifiers)
- **THEN** query gains the character `a`
- **AND** no pane is added to the list

#### Scenario: Letter `c` filters instead of closing
- **GIVEN** mode is `Filter`
- **WHEN** the user presses `c` (no modifiers)
- **THEN** query gains the character `c`
- **AND** the plugin does NOT close
- **AND** users wanting to close from filter mode must press `Esc` to drop to command mode (clearing query if non-empty), then `c` (or just `Esc` again from command)

#### Scenario: Digit `1` filters instead of jumping
- **GIVEN** mode is `Filter`
- **WHEN** the user presses `1` (no modifiers)
- **THEN** query gains the character `1`
- **AND** no jump occurs

### Requirement: Filter mode rejects modified printable keys

In `Filter` mode, printable characters SHALL be appended to the query ONLY when the modifier set is **empty `{}`** (post-FFI normalization). Per the FFI normalization rule (see `design.md` "Modifier-gated key consumption with FFI normalization"), Shift+letter inputs are collapsed at the FFI boundary into uppercase-with-empty-modifier-set BEFORE reaching the handler, so by the time `handle_filter_key` sees an input, Shift on letters is already gone. The empty-modifier gate is therefore sufficient to accept both `a` and `A` (the post-normalization shape of Shift+`a`).

Inputs with `Ctrl`, `Alt`, or `Super` modifiers SHALL be a no-op (return `vec![Effect::Noop]`); they SHALL NOT append to the query, SHALL NOT trigger a re-filter, and SHALL NOT be silently swallowed.

This prevents standard terminal expectations (`Ctrl+W` word-delete, `Ctrl+C` cancel, `Alt+<x>` host-bound shortcuts) from being captured as query input.

#### Scenario: Ctrl+W in filter mode is a no-op
- **GIVEN** mode is `Filter` and query is `"ed"`
- **WHEN** the user presses `Ctrl+w`
- **THEN** query remains `"ed"`
- **AND** no `Effect::Render` is emitted
- **AND** the handler returns `vec![Effect::Noop]`

#### Scenario: Shift+letter (uppercase) in filter mode appends
- **GIVEN** mode is `Filter` and query is `""`
- **WHEN** the user presses Shift+`a`
- **AND** the FFI normalization layer rewrites the input to `Char('A')` with an EMPTY modifier set (per the modifier-gating decision in `design.md`)
- **THEN** the post-normalization input arrives at `handle_filter_key` as `InputKey::Char('A', ModifierSet::default())`
- **AND** the handler appends `'A'` to the query (modifier set is empty, so the gate passes)
- **AND** query becomes `"A"`
- **AND** the handler returns `vec![Effect::Render]`

#### Scenario: Alt+letter is not consumed
- **GIVEN** mode is `Filter` and query is `"e"`
- **WHEN** the user presses `Alt+d`
- **THEN** query remains `"e"`
- **AND** the handler returns `vec![Effect::Noop]`

#### Scenario: Color levels for highlight do not collide with hint colors
- **GIVEN** mode is `Filter`, query is `"ed"`, and a row matches
- **WHEN** the plugin renders
- **THEN** match highlight indices use `Text` color level `1`
- **AND** hint-line key labels use color level `3` (existing)
- **AND** mode badge accent uses color level `2`

### Requirement: Selection survives pane updates during active filter

While mode is `Filter` AND query is non-empty, `update_panes()` (the handler for `PaneUpdate`/`TabUpdate` events) SHALL NOT overwrite `State.selected` from the focused-pane index. Specifically, the focused-pane → `selected` reconciliation that runs in `Command` mode SHALL be gated on `mode == Command || (mode == Filter && query.is_empty())`. After any underlying `panes` mutation, `selected` SHALL be clamped against the current filtered view length so it never exceeds `filtered_indices().len().saturating_sub(1)`.

#### Scenario: Pane update during filter does not move selected
- **GIVEN** mode is `Filter`, query is `"ed"`, filtered view has 2 rows, `selected = 1`
- **AND** focused pane is at index 5 in the full list
- **WHEN** a `PaneUpdate` event fires that does not change the filtered view
- **THEN** `selected` remains `1`

#### Scenario: Filtered view shrinks below selected
- **GIVEN** mode is `Filter`, query is `"ed"`, filtered view has 3 rows, `selected = 2`
- **WHEN** a pane update reduces the filtered view to 1 row
- **THEN** `selected` is clamped to `0`
- **AND** rendering does not panic

#### Scenario: Selected re-anchors when query becomes empty
- **GIVEN** mode is `Filter`, query is `"ed"`, `selected = 0`
- **AND** focused pane is at index 2 in the full list
- **WHEN** the user backspaces until query is `""`
- **THEN** `selected` re-anchors to the focused pane's index in the full list

### Requirement: Every filter-mode key path triggers a render

Every `handle_filter_key` code path that mutates `State.query`, `State.selected`, or `State.mode` SHALL cause `update()` to return `should_render = true` so that the visible UI reflects the keystroke before the next pane event.

#### Scenario: Typing a character re-renders the filtered list
- **GIVEN** mode is `Filter` and query is `"ed"`
- **WHEN** the user presses a printable key that mutates `State.query`
- **THEN** `handle_filter_key` returns an effect set including `Effect::Render`
- **AND** `update()` returns `should_render = true` for that event

