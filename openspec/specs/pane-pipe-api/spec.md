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
call it invokes — including `ReadCliPipes` (required for
`unblock_cli_pipe_input` / `cli_pipe_output`), `OpenTerminalsOrPlugins`
(required for the respawn branch's plugin-pane spawn), and
`MessageAndLaunchOtherPlugins` (required for the `bootstrap_store` hand-off
send) — so that no host call the plugin depends on is silently
permission-denied at runtime.

#### Scenario: ReadCliPipes requested at load
- **WHEN** the plugin's `load()` runs
- **THEN** the permission request SHALL include `ReadCliPipes` alongside the
  existing `RunCommands`, `ReadApplicationState`, and
  `ChangeApplicationState` permissions

#### Scenario: MessageAndLaunchOtherPlugins requested at load
- **WHEN** the plugin's `load()` runs
- **THEN** the permission request SHALL include
  `MessageAndLaunchOtherPlugins` and `OpenTerminalsOrPlugins`

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
- **THEN** the plugin SHALL NOT treat any gated host-call behavior
  (pipe-release, pipe-output, plugin-pane spawn, bootstrap send) as
  available (grant is verified, never assumed — Constitution IV)

---

### Requirement: Toggle Pipe Invocation

THE plugin SHALL expose a named `toggle` pipe that drives its show/hide
lifecycle, so that invocation never depends on the host's
`LaunchOrFocusPlugin` focus path. On receiving a `toggle` pipe message the
plugin SHALL select exactly one of four behavioral branches from
synchronously queried host state (never from cached events):

1. visible → hide (`hide_self()`; Esc-close semantics unchanged);
2. hidden and parked on the client's active tab → show in place, warm
   (`show_self`);
3. hidden and parked on another tab (or parked on an unknown tab) →
   RESPAWN on the invoking tab: open a fresh instance of itself as a
   floating pane in the client's active tab
   (`open_plugin_pane_floating(own_url, own_configuration)` — a new-pane
   host action), then close the old instance (`close_self()`). Pane
   relocation via `break_panes_to_tab_with_index` is FORBIDDEN: under
   tab-id/position drift that host call destroys the extracted pane
   (upstream defect, evidence 2026-07-11);
4. freshly spawned by the pipe itself (cold spawn, no cached event state) →
   show in place.

The respawned instance SHALL be opened with the plugin's own URL and its
verbatim load-time configuration, so the new instance remains the pipe
destination of the invoking keybind (instance identity = URL +
configuration).

Branch selection SHALL be pure decision logic in `harpoon-core` (Constitution
I core/shim split); only the host calls live in the plugin shim.

#### Scenario: same-tab re-invoke after Esc-close
- **GIVEN** the plugin was shown and then hidden via Esc on tab T
- **WHEN** a `toggle` pipe message arrives while the client's active tab is T
- **THEN** the plugin SHALL show itself as a floating pane on tab T
- **AND** the client view SHALL remain on tab T

#### Scenario: cross-tab invoke respawns on the invoking tab
- **GIVEN** the plugin is hidden and parked on tab A
- **WHEN** a `toggle` pipe message arrives while the client's active tab is
  B (B ≠ A)
- **THEN** the plugin SHALL open a fresh instance of itself as a floating,
  focused pane on tab B and close the old instance
- **AND** the menu SHALL end as a floating pane on tab B with the client
  view remaining on tab B (never hopping through tab A)
- **AND** exactly one plugin instance SHALL survive, and it SHALL remain
  reachable by the invoking keybind's pipe (identical URL + configuration)

#### Scenario: correct under tab-id/position drift
- **GIVEN** at least one tab has been closed since the plugin's home tab was
  created (tab ids drifted above positions)
- **WHEN** a `toggle` pipe message arrives from any tab
- **THEN** the menu and the client view SHALL end on the invoking tab (never
  on a tab addressed by a stale tab id)

#### Scenario: visible focused toggle hides
- **GIVEN** the plugin is visible AND is the client's focused pane (the
  reachable "open in front of the user" state — showing a floating plugin
  focuses it, and focusing elsewhere hides the floating layer)
- **WHEN** a `toggle` pipe message arrives
- **THEN** the plugin SHALL hide itself (`hide_self()`), preserving the
  mode-state-machine Close consolidation behavior

#### Scenario: unfocused container state is shown, not hidden
- **GIVEN** the plugin's pane is in a tiled/floating container
  (unsuppressed) but is NOT the client's focused pane — e.g. a
  pipe-cold-spawned pane parked floating and unfocused (evidence
  2026-07-11: such a pane is indistinguishable from a user-visible one via
  synchronous queries, and hiding it made the first invocation a visible
  no-op)
- **WHEN** a `toggle` pipe message arrives
- **THEN** the plugin SHALL bring its menu to the user (show + focus — in
  place when already on the active tab, else via the respawn branch), never
  hide it

#### Scenario: cold spawn shows without cached event state
- **GIVEN** the plugin is not loaded and the `toggle` pipe message spawns it
  (the message arrives before the first `TabUpdate`/`PaneUpdate`)
- **WHEN** the toggle handler runs
- **THEN** the plugin SHALL show itself without reading cached
  `TabUpdate`/`PaneUpdate` state (Constitution IV — never act on unverified
  host state)

#### Scenario: toggle pipe client released
- **WHEN** a CLI-sourced `toggle` pipe message is handled
- **THEN** the plugin SHALL unblock that pipe input after handling (per the
  Cli Pipe Client Release requirement's exactly-once discipline)

### Requirement: Toggle State Is Sync-Query-Verified

THE plugin SHALL establish the state driving a `toggle` decision — its own
pane's suppressed/visible state, its own URL, and the invoking client's
focused tab — via synchronous host queries at pipe-handling time
(`get_pane_info(PaneId::Plugin(own))`; `get_focused_pane_info()` returning
the stable tab ID), and SHALL NOT derive that state from cached
`TabUpdate`/`PaneUpdate` events or from `Event::Visible` (probe evidence
2026-07-11: event caches freeze while the pane is suppressed, and zellij
emits `Event::Visible` only to TILED plugin panes — a floating plugin never
receives it). The parked-tab record used for the same-tab-vs-cross-tab
determination SHALL originate from synchronous queries taken ONLY at
moments the plugin's own pane is verifiably the client's focused pane
(post-show, pre-hide, or a grant-time check that passes the focused-pane
identity test) — never from event caches and never from a focused-tab
sample taken while another pane holds focus (a `jump_pane` cold spawn
parks the pane on one tab while focusing a terminal on another; a proxy
sample would poison the record and re-create the wrong-tab symptom).
When no verified record exists the plugin SHALL take the respawn branch
rather than show in place on a guess. Constitution IV: never act on
unverified host state.

#### Scenario: suppressed state queried, not assumed
- **GIVEN** the plugin has been hidden long enough for cached events to be
  stale (events do not flow to suppressed panes)
- **WHEN** a `toggle` pipe message arrives
- **THEN** the visibility decision SHALL come from a synchronous
  `get_pane_info` query of the plugin's own pane (fresh `is_suppressed`)

#### Scenario: cross-tab determination queried, not cached
- **GIVEN** the client switched tabs while the plugin was hidden (cached
  `TabUpdate` still reports the pre-hide active tab)
- **WHEN** a `toggle` pipe message arrives from the new tab
- **THEN** the same-tab-vs-cross-tab determination SHALL compare the
  synchronously queried focused tab identity (`get_focused_pane_info`)
  against the sync-recorded parked tab, never against the cached active tab

#### Scenario: jump-spawned instance does not poison the parked record
- **GIVEN** a cold `jump_pane` pipe spawned the plugin parked on tab A
  while focusing a terminal pane on tab B (focus never on the plugin)
- **WHEN** a `toggle` pipe message later arrives from tab B
- **THEN** the menu SHALL end on tab B with the view on tab B (no verified
  parked record exists → respawn branch), never a warm in-place show that
  yanks the view to tab A

### Requirement: Respawn State Hand-Off

THE outgoing instance SHALL hand its in-memory bookmark state directly to
the successor WHEN the `toggle` pipe's respawn branch opens a fresh
instance on the invoking tab, instead of leaving the successor to race the
disk load: it SHALL capture the successor's pane id from the spawn host call's
return value, send a `bootstrap_store` pipe message routed by destination
plugin id (never by url+configuration matching) whose payload carries the
serialized bookmark store (persistence v2 envelope) and the session name,
and only then close itself. The successor SHALL adopt the payload as its
in-memory store immediately on receipt. Adoption SHALL be deny-safe pure
state mutation only — no response-decoding host calls — because the
bootstrap message can arrive before the successor's permission grant.

Adoption-vs-disk precedence SHALL be pure decision logic in `harpoon-core`
(Constitution I): an adopted bootstrap payload wins while the successor's
disk load is unresolved; a disk load result arriving after adoption SHALL
reconcile and SHALL NOT clobber newer in-memory mutations. The disk load
remains the cold-boot fallback whenever no bootstrap payload arrives.

#### Scenario: cross-tab respawn presents persisted targets immediately
- **GIVEN** the plugin is hidden and parked on tab A with a non-empty
  bookmark store
- **WHEN** a `toggle` pipe message arrives while the client's active tab is
  B (B ≠ A) and the respawn branch runs
- **THEN** the successor's menu SHALL present every persisted jump-target
  row on its first actual render (live pane where resolved, saved
  `tab_name | pane_title` placeholder where the host manifest is not ready,
  including persisted post-freeze bookmarks with `index=None`)
- **AND** rendering SHALL remain suppressed rather than expose an empty or
  partial projection (no invoke-again-to-recover)

#### Scenario: bootstrap arriving pre-grant is adopted safely
- **GIVEN** the successor has loaded but its permission grant has not yet
  been confirmed (`PermissionRequestResult` not received)
- **WHEN** the `bootstrap_store` pipe message arrives
- **THEN** the successor SHALL adopt the payload via pure state mutation
  only, making no response-decoding host call in the adoption path
- **AND** the plugin SHALL NOT panic

#### Scenario: late disk load reconciles without clobbering newer mutations
- **GIVEN** the successor adopted a bootstrap payload and the user then
  mutated the store (e.g. added a bookmark)
- **WHEN** the successor's independently initiated disk load result arrives
  afterwards
- **THEN** the in-memory store SHALL retain the adopted state and user's
  newer mutation verbatim
- **AND** the disk result SHALL be consumed as the persistence baseline for
  destructive-save comparison (reconciliation input), never replace or
  append stale rows into newer in-memory state

#### Scenario: spawn id unavailable degrades to disk load
- **IF** the spawn host call returns no pane id (or a non-plugin id)
- **THEN** the outgoing instance SHALL skip the bootstrap send and close
  itself as today
- **AND** the successor SHALL populate its store via the existing disk-load
  path (behavior never worse than the status quo)

#### Scenario: unavailable hand-off degrades safely
- **IF** payload encoding fails after a successor pane id was returned
- **THEN** the outgoing instance SHALL skip the bootstrap send, close, and
  the successor SHALL fall back to its independently initiated disk load

#### Scenario: aggregate permission denial is deny-safe
- **IF** zellij denies the aggregate permission request (which includes both
  `OpenTerminalsOrPlugins` and `MessageAndLaunchOtherPlugins`)
- **THEN** the plugin SHALL invoke no gated response-decoding host call (no
  panic) and SHALL use the deny-safe show-in-place fallback
- **AND** because aggregate denial includes `ChangeApplicationState`, zellij
  may suppress the denied plugin after prompt teardown; harpoon SHALL remain
  inert/alive while the user's terminal remains visible (it SHALL NOT attempt
  an ungranted post-denial show)
- **AND** queued CLI messages SHALL remain unexecuted rather than assume the
  denied query/output/unblock capabilities

### Requirement: Duplicate Toggle Delivery Tolerance

THE successor SHALL NOT hide its just-shown menu in response to a stale
toggle: IF a queued or stale `toggle` pipe message is re-delivered to a
freshly respawned successor before the user has interacted with the
just-shown menu, THEN the successor SHALL ignore it while still releasing
the blocked CLI client (probe evidence 2026-07-13: the in-flight CLI
invocation pipe that triggered the respawn is re-delivered to the successor
~380ms after load, carrying the SAME pipe id). The ignore-vs-honor decision
SHALL be pure decision logic in `harpoon-core`, keyed on deterministic pipe
identity — the hand-off payload carries the pipe id the sender already
handled and the successor ignores exactly one toggle from that source —
never on wall-clock debounce (and never on a shown/readiness proxy: the
re-delivery arrives AFTER the menu is shown, so timing/readiness conditions
cannot distinguish it from a genuine re-invoke).

#### Scenario: re-delivered invocation pipe does not hide the menu
- **GIVEN** the respawn branch just spawned a successor and the successor
  has shown its menu
- **WHEN** the original in-flight `toggle` pipe message is re-delivered to
  the successor immediately after load (same CLI pipe id the outgoing
  instance already handled)
- **THEN** the menu SHALL remain shown (the stale toggle is ignored)
- **AND** the blocked CLI pipe client SHALL still be released

#### Scenario: genuine user re-invoke still hides
- **GIVEN** the successor's menu is shown
- **WHEN** the user presses the keybind again (a new `toggle` pipe message —
  keybind-sourced or a NEW CLI pipe id, never the sender-handled id)
- **THEN** the visible-and-focused hide branch SHALL run as specified by
  Toggle Pipe Invocation

