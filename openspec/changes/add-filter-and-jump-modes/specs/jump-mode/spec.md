## ADDED Requirements

### Requirement: Slot mapping

Slots SHALL be addressable as `1` through `9` mapping to `panes[0]` through `panes[8]`, then `a` through `z` mapping to `panes[9]` through `panes[34]`. Panes at index 35 or greater SHALL have NO slot shortcut. The slot mapping SHALL be a pure function of the pane Vec position.

#### Scenario: Digit slots address the first nine panes
- **GIVEN** `State.panes` has at least 9 entries
- **WHEN** the slot mapping is queried for character `5`
- **THEN** it resolves to `panes[4]`

#### Scenario: Letter slots address panes 10 through 35
- **GIVEN** `State.panes` has at least 11 entries
- **WHEN** the slot mapping is queried for character `b`
- **THEN** it resolves to `panes[10]`

#### Scenario: Slot beyond list length is unaddressable
- **GIVEN** `State.panes` has 3 entries
- **WHEN** the slot mapping is queried for character `5`
- **THEN** the lookup returns no pane

#### Scenario: Index 35+ has no slot
- **GIVEN** `State.panes` has 40 entries
- **WHEN** all slot keys (`1-9`, `a-z`) are queried
- **THEN** none resolves to `panes[35]` or beyond

### Requirement: Jump mode is read-only with respect to harpoon state

In `Jump` mode, ALL keys other than slot characters (`1-9`, `a-z`) and `Esc` SHALL be ignored. No key in `Jump` mode SHALL mutate `State.panes`, persistence, or any harpoon-internal state besides `mode` (via `Esc`). Focusing the target pane via `focus_terminal_pane` is the sole external side effect, since that is the operation's purpose.

#### Scenario: Letter `d` does not delete in jump mode
- **GIVEN** mode is `Jump` and a pane is at `selected`
- **WHEN** the user presses `d`
- **THEN** `d` resolves as a slot lookup (slot `d` = `panes[12]`) if present
- **AND** if `panes[12]` exists, jump fires
- **AND** if `panes[12]` does NOT exist, the keypress is ignored
- **AND** no pane is deleted in either case

#### Scenario: Letter `a` does not add in jump mode
- **GIVEN** mode is `Jump`
- **WHEN** the user presses `a`
- **THEN** `a` resolves as slot lookup (slot `a` = `panes[9]`) if present
- **AND** no pane is added

#### Scenario: Arrow keys are ignored in jump mode
- **GIVEN** mode is `Jump`
- **WHEN** the user presses an arrow key
- **THEN** `selected` does not change
- **AND** mode remains `Jump`

#### Scenario: Backspace is a no-op in jump mode
- **GIVEN** mode is `Jump`
- **WHEN** the user presses `Backspace`
- **THEN** mode remains `Jump`
- **AND** no state changes

### Requirement: Jump fires and closes

When a slot key resolves to an existing pane in `Jump` mode (or in `Command` mode — see below), the plugin SHALL focus that pane and close via the standard close helper. When a slot key does NOT resolve to an existing pane, the keypress SHALL be a no-op.

#### Scenario: Slot key fires jump and closes from jump mode
- **GIVEN** mode is `Jump` and `panes[2]` exists
- **WHEN** the user presses `3`
- **THEN** `focus_terminal_pane(panes[2].pane_info.id, true)` is invoked
- **AND** the plugin closes
- **AND** mode resets to `default_mode`

#### Scenario: Empty slot is a no-op
- **GIVEN** mode is `Jump` and `State.panes` has 2 entries
- **WHEN** the user presses `5`
- **THEN** no jump fires
- **AND** the plugin remains open
- **AND** mode remains `Jump`

### Requirement: Digit-only jumps from command mode

`Command` mode SHALL accept ONLY digit slot keys (`1-9`) as immediate jump triggers, with the same semantics as `Jump` mode. Letter slot keys (`a-z`) SHALL retain their existing command-mode meanings (`a` = add focused, `A` = add all, `d` = delete, `j`/`k` = nav, `l` = focus selected, `c` = close); to use letter slots the user MUST first enter `Jump` mode via `#`.

This restriction exists because letters `a`, `c`, `d`, `j`, `k`, `l` are already bound to commands in `Command` mode; allowing them to also fire jumps would require breaking existing bindings or producing ambiguous behavior. Digits `1-9` are unbound in command mode today and have no collision.

#### Scenario: Digit jumps from command mode
- **GIVEN** mode is `Command` and `panes[0]` exists
- **WHEN** the user presses `1`
- **THEN** `focus_terminal_pane(panes[0].pane_info.id, true)` is invoked
- **AND** the plugin closes

#### Scenario: Letter does NOT jump from command mode
- **GIVEN** mode is `Command` and `panes[10]` exists (slot `b`)
- **WHEN** the user presses `b`
- **THEN** `focus_terminal_pane` is NOT invoked
- **AND** mode remains `Command`
- **AND** the keypress is ignored (no command bound to `b` either)

#### Scenario: Letter command keys retain their command bindings
- **GIVEN** mode is `Command` and a pane is `selected`
- **WHEN** the user presses `d`
- **THEN** the selected pane is deleted (existing command-mode behavior)
- **AND** no slot lookup occurs

#### Scenario: Letter slots reachable via Jump mode
- **GIVEN** mode is `Command` and `panes[10]` exists
- **WHEN** the user presses `#` then `b`
- **THEN** mode transitions to `Jump`
- **AND** `focus_terminal_pane(panes[10].pane_info.id, true)` is invoked
- **AND** the plugin closes

#### Scenario: Empty digit slot in command mode is a no-op
- **GIVEN** mode is `Command` and `State.panes` has 2 entries
- **WHEN** the user presses `5`
- **THEN** no jump fires
- **AND** the plugin remains open
- **AND** mode remains `Command`

### Requirement: Placeholder slots during partial restore

`State.panes` SHALL be a sparse `Vec<Option<Pane>>` during the partial-restore window. When `Persistence::bookmarks` contains entries with `index = Some(i)` whose `(tab_name, pane_title)` has NOT yet been observed in any `PaneManifest`, `State.panes[i] = None` and the rendered list SHALL display a **placeholder row** at index `i`. Saved positions remain stable through the restore window: live panes occupy `Some(_)` slots at their saved indices, and `None` gaps are rendered as placeholders.

Placeholder rows SHALL render as `<slot>  ?  (resolving)` (or `   ?  (resolving)` when `show_slots = false`) so the user sees that a slot is reserved but not yet ready.

Slot keys (digits in `Command` mode, digits + letters in `Jump` mode) that resolve to a `None` slot SHALL be a **no-op** — no pane is focused, the plugin remains open, and the keypress returns `vec![]`. This guarantees that pressing `2` always jumps to the pane the user pinned at slot 2, OR no-ops if slot 2 hasn't yet resolved — it never jumps to a different pane that happens to be in `panes[1]` because of compaction.

In `Filter` mode, `None` slots SHALL be excluded from the filtered view entirely (the matcher only iterates `Some` entries).

On the first user mutation (`a`/`A`/`d`/`K`/`J` with a non-no-op outcome), `State.panes` SHALL be **compacted** via `freeze_on_user_mutation` (drop all `None` entries), and unresolved bookmarks with `index = Some(_)` SHALL have their `index` rewritten to `None`. Post-freeze, `State.panes` is dense for the rest of the session; placeholders are no longer present in any slot.

#### Scenario: Placeholder rows render at unresolved saved indices
- **GIVEN** persistence loaded 3 bookmarks at saved indices `0`, `1`, `2`
- **AND** in the first `update_panes` round only the bookmark at saved index `0` is currently visible
- **AND** mode is `Command` and `show_slots = true`
- **WHEN** the plugin renders
- **THEN** row 0 displays `"1  <tab> | <title>"` (the live pane)
- **AND** row 1 displays `"2  ?  (resolving)"` (placeholder)
- **AND** row 2 displays `"3  ?  (resolving)"` (placeholder)

#### Scenario: Slot key on placeholder is a no-op
- **GIVEN** mode is `Command`, `panes[0]` is live, slot 2 is a placeholder, slot 3 is a placeholder
- **WHEN** the user presses `2`
- **THEN** the handler returns `vec![]`
- **AND** the plugin remains open
- **AND** no `focus_terminal_pane` call is made

#### Scenario: Placeholder slot key in jump mode is a no-op
- **GIVEN** mode is `Jump` and slot `b` is a placeholder
- **WHEN** the user presses `b`
- **THEN** the handler returns `vec![]`
- **AND** mode remains `Jump`
- **AND** no jump fires

#### Scenario: Placeholder resolves to live row when bookmark materializes
- **GIVEN** slot 2 currently renders as a placeholder (`"2  ?  (resolving)"`)
- **WHEN** a `PaneUpdate` event arrives that includes the matching `(tab_name, pane_title)`
- **AND** `update_panes` runs the restore resolution loop
- **THEN** `state.panes` now contains the matching `Pane` at index 1
- **AND** the next render shows row 1 as `"2  <tab> | <title>"` (live, no longer a placeholder)

#### Scenario: Placeholder excluded from filter view
- **GIVEN** persistence has 3 bookmarks; only saved index 0 is currently resolved
- **AND** slot 2 and slot 3 are placeholders
- **AND** mode is `Filter` with query `"x"`
- **WHEN** the plugin renders
- **THEN** placeholder bookmarks are NOT considered for matching
- **AND** the filtered view contains only live panes

#### Scenario: Freeze on user mutation collapses placeholders to append
- **GIVEN** persistence has 3 bookmarks at saved indices `0`, `1`, `2`; only saved index 0 is resolved
- **AND** slot 2 and slot 3 are placeholders
- **WHEN** the user presses `K` (or any other mutation key) while `selected = 0`
- **THEN** every unresolved bookmark with `index = Some(_)` has its `index` rewritten to `None`
- **AND** the placeholder rows for slot 2 and slot 3 are no longer rendered
- **AND** when the unresolved bookmarks eventually become visible, they append at the end of `state.panes` rather than seek their old saved indices

### Requirement: Slot prefix rendering in command and jump modes

In `Command` and `Jump` modes, each rendered row SHALL be prefixed with the slot character followed by two spaces (3 chars total) when `show_slots = true` (default). Rows without a slot (index 35+) SHALL render with three spaces of padding to keep alignment. When `show_slots = false`, no prefix SHALL be rendered.

In `Filter` mode the slot prefix SHALL be SUPPRESSED, because (a) filtered rows are reordered by match score and slot prefixes would no longer correspond to row position, and (b) pressing a prefix character in filter mode appends to the query rather than firing a jump, which would be a UX trap.

#### Scenario: Slot prefix shown in command mode
- **GIVEN** `show_slots = true` and mode is `Command`
- **AND** `panes[0]` displays as `"work | nvim"`
- **WHEN** the plugin renders
- **THEN** the row is rendered as `"1  work | nvim"`

#### Scenario: Slot prefix shown in jump mode
- **GIVEN** `show_slots = true` and mode is `Jump`
- **AND** `panes[10]` displays as `"build | cargo"`
- **WHEN** the plugin renders
- **THEN** the row is rendered as `"b  build | cargo"`

#### Scenario: Slot prefix suppressed in filter mode
- **GIVEN** `show_slots = true` and mode is `Filter`
- **AND** `panes[10]` displays as `"build | cargo"`
- **AND** the pane is in the filtered view
- **WHEN** the plugin renders
- **THEN** the row is rendered as `"build | cargo"` (no prefix)

#### Scenario: Beyond-slot rows render with padding (command/jump)
- **GIVEN** `show_slots = true` and mode is `Command`
- **AND** `panes[35]` exists with display `"old | tail"`
- **WHEN** the plugin renders
- **THEN** the row is rendered as `"   old | tail"`

#### Scenario: show_slots disabled removes prefix in all modes
- **GIVEN** `show_slots = false`
- **AND** `panes[0]` displays as `"work | nvim"`
- **WHEN** the plugin renders in any mode
- **THEN** the row is rendered as `"work | nvim"`
