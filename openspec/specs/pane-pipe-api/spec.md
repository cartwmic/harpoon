# pane-pipe-api Specification

## Purpose
TBD - created by archiving change ntfy-harpoon-jump. Update Purpose after archive.
## Requirements
### Requirement: Pane Id String Parsing

THE plugin SHALL parse a zellij pane-id string into a terminal pane id `u32`,
accepting both the `terminal_N` form exported as `$ZELLIJ_PANE_ID` and the bare
integer `N` form.

#### Scenario: terminal-prefixed form
- **WHEN** the pipe payload is the string `terminal_7`
- **THEN** the plugin SHALL resolve it to terminal pane id `7`

#### Scenario: bare integer form
- **WHEN** the pipe payload is the string `7`
- **THEN** the plugin SHALL resolve it to terminal pane id `7`

#### Scenario: unparseable payload
- **IF** the pipe payload is empty or does not match `terminal_N` or a bare
  integer (e.g. `plugin_3`, `abc`)
- **THEN** the plugin SHALL treat the request as having no resolvable pane id and
  SHALL NOT mutate harpoon state or focus any pane

### Requirement: Slot For Pane Reverse Lookup

THE plugin SHALL answer a CLI pipe message named `slot_for_pane` by returning,
via `cli_pipe_output`, the 1-based harpoon slot holding the given pane id, and
SHALL NOT mutate harpoon state.

#### Scenario: harpooned pane
- **WHEN** a `slot_for_pane` pipe carries a pane id whose bookmark is materialized
  at 0-based slot index `2` (`PaneBookmark.index`)
- **THEN** the plugin SHALL emit the string `3` on the CLI pipe output

#### Scenario: un-harpooned pane
- **IF** a `slot_for_pane` pipe carries a pane id that has no bookmark in the
  harpoon store, or whose bookmark has no materialized slot index
  (`PaneBookmark.index` is `None`)
- **THEN** the plugin SHALL emit an empty string on the CLI pipe output and leave
  harpoon state unchanged

### Requirement: Jump To Pane By Id

THE plugin SHALL answer a CLI pipe message named `jump_pane` carrying a
resolvable pane id by focusing that terminal pane and its tab using the existing
deterministic fullscreen-safe jump path, so the outcome is correct in both plain
and stacked fullscreen layouts.

#### Scenario: jump focuses the target pane
- **WHEN** a `jump_pane` pipe carries a resolvable terminal pane id present in
  the live session
- **THEN** the plugin SHALL focus that terminal pane and its containing tab

#### Scenario: fullscreen normalization reused
- **WHILE** the target pane's tab is in fullscreen mode
- **WHEN** a `jump_pane` pipe targets that pane
- **THEN** the plugin SHALL drive focus through the existing
  `jump_focus_fullscreen` two-phase path rather than the superseded
  `PaneInfo.is_fullscreen` tab-level heuristic

#### Scenario: unresolvable target
- **IF** a `jump_pane` pipe carries a payload that does not resolve to a pane id
- **THEN** the plugin SHALL NOT change focus or fullscreen state

---

