# Capability: pane-pipe-api

## ADDED Requirements

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
determination SHALL itself originate from synchronous queries taken at the
moments the parking changes (load, hide) — never from event caches.
Constitution IV: never act on unverified host state.

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

## MODIFIED Requirements

## REMOVED Requirements

## RENAMED Requirements

---

## Acceptance criterion quality checklist

| AC ID | Testable | Solution-free | Unambiguous | Consistent | Complete |
|---|---|---|---|---|---|
| pane-pipe-api.toggle-pipe-invocation | [x] | [x] | [x] | [x] | [x] |
| pane-pipe-api.toggle-state-sync-query-verified | [x] | [x] | [x] | [x] | [x] |
