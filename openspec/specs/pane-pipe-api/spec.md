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
resolvable pane id by focusing that terminal pane and its tab and leaving the
target pane fullscreen, with the outcome correct in every combination of
plain/stacked fullscreen layout, same-tab/cross-tab origin, and warm/cold
plugin state cache.

#### Scenario: jump focuses the target pane
- **WHEN** a `jump_pane` pipe carries a resolvable terminal pane id present in
  the live session
- **THEN** the plugin SHALL focus that terminal pane and its containing tab
- **AND** the target pane SHALL end fullscreen in its tab

#### Scenario: cross-tab jump into a fullscreen tab
- **WHILE** the target pane's tab is inactive and fullscreen on a different
  pane
- **WHEN** a `jump_pane` pipe targets a hidden pane of that tab
- **THEN** the plugin SHALL end with the target pane focused and fullscreen

#### Scenario: cold-start pipe into a fullscreen tab
- **WHEN** a `jump_pane` pipe is delivered to a freshly loaded instance (empty
  state caches) targeting a pane in a fullscreen tab
- **THEN** the plugin SHALL end with the target pane focused and fullscreen

#### Scenario: unresolvable target
- **IF** a `jump_pane` pipe carries a payload that does not resolve to a pane
  id
- **THEN** the plugin SHALL NOT change focus or fullscreen state

---

### Requirement: Ground Truth Fullscreen Normalization

WHEN a jump requires a fullscreen toggle decision, THE plugin SHALL derive the
tab's fullscreen state from ground truth current at decision time (a fresh
post-focus query of the host, or a state deterministically established by an
immediately preceding step of the same jump sequence) — never from
event-cached `TabInfo`/`PaneInfo` snapshots or from predictions of the host's
focus side effects.

#### Scenario: cold-start pipe decides from ground truth
- **WHEN** a `jump_pane` pipe is delivered to a freshly loaded plugin instance
  whose `TabUpdate`/`PaneUpdate` caches are still empty
- **THEN** the plugin SHALL resolve the target tab's actual fullscreen state
  before issuing any fullscreen toggle
- **AND** the jump outcome SHALL be identical to the warm-cache outcome

#### Scenario: no toggle from unknown state
- **IF** the tab's current fullscreen state cannot be established
- **THEN** the plugin SHALL NOT issue a fullscreen toggle for that jump

### Requirement: Targeted Pipe Delivery

THE plugin's documentation and any shipped invocation examples SHALL instruct
callers to target `jump_pane` pipes at the plugin explicitly (`--plugin`,
optionally with `--plugin-configuration`), and SHALL NOT recommend broadcast
pipes (no `--plugin` target) for `jump_pane`.

#### Scenario: README example is targeted
- **WHEN** the README documents invoking `jump_pane` over the CLI pipe
- **THEN** the example SHALL pass an explicit `--plugin` target

#### Scenario: broadcast hazard documented
- **IF** more than one harpoon instance is loaded (distinct configurations)
- **THEN** the documentation SHALL state that a broadcast `jump_pane` reaches
  all instances and risks fullscreen double-toggle cancellation

### Requirement: Cli Pipe Client Release

THE plugin SHALL unblock a CLI pipe's input exactly once after handling each
CLI-sourced pipe message (`jump_pane`, `slot_for_pane`, or an unrecognized
name), so the invoking `zellij pipe` client process terminates without
depending on the host's implicit release.

#### Scenario: jump_pane client terminates
- **WHEN** a CLI `jump_pane` pipe message is handled (resolvable or not)
- **THEN** the plugin SHALL unblock the `jump_pane` pipe input after handling
- **AND** the invoking client process SHALL terminate

#### Scenario: unrecognized pipe name still released
- **IF** a CLI pipe message arrives with a name the plugin does not handle
- **THEN** the plugin SHALL still unblock that pipe input (no-op handling
  never strands the client)

#### Scenario: non-CLI pipes unaffected
- **WHEN** a pipe message arrives from a non-CLI source (plugin-to-plugin)
- **THEN** the plugin SHALL NOT issue a CLI-pipe unblock for it

---

### Requirement: Host Call Permission Completeness

THE plugin SHALL request, at load, every `PermissionType` required by a host
call it invokes — including `ReadCliPipes`, which zellij requires for
`unblock_cli_pipe_input` and `cli_pipe_output` — so that no host call the
plugin depends on is silently permission-denied at runtime.

#### Scenario: ReadCliPipes requested at load
- **WHEN** the plugin's `load()` runs
- **THEN** the permission request SHALL include `ReadCliPipes` alongside the
  existing `RunCommands`, `ReadApplicationState`, and
  `ChangeApplicationState` permissions

#### Scenario: pipe host calls not denied after grant
- **WHILE** `ReadCliPipes` has been granted
- **WHEN** a CLI-sourced pipe message (`jump_pane`, `slot_for_pane`, or an
  unrecognized name) is handled
- **THEN** the resulting `unblock_cli_pipe_input` / `cli_pipe_output` host
  calls SHALL NOT be permission-denied (the zellij server log SHALL gain no
  `ReadCliPipes' denied` lines)
- **AND** the invoking `zellij pipe` client process SHALL exit promptly
  (no hang, no `timeout` exit 124)

#### Scenario: ungranted permission is not assumed
- **IF** the host reports the permission request denied
  (`PermissionRequestResult`)
- **THEN** the plugin SHALL NOT treat the gated pipe-release and pipe-output
  behavior as available (grant is verified, never assumed — Constitution IV)

