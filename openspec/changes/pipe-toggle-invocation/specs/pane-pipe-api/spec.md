# Capability: pane-pipe-api

## ADDED Requirements

### Requirement: Toggle Pipe Invocation

THE plugin SHALL expose a named `toggle` pipe that drives its show/hide
lifecycle, so that invocation never depends on the host's
`LaunchOrFocusPlugin` focus path. On receiving a `toggle` pipe message the
plugin SHALL select exactly one of four behavioral branches from
synchronously queried host state (never from cached events):

1. visible → hide (`hide_self()`; Esc-close semantics unchanged);
2. hidden and parked on the client's active tab → show in place
   (`show_self`);
3. hidden and parked on another tab → un-suppress via `show_self` FIRST,
   THEN relocate its own pane to the active tab
   (`break_panes_to_tab_with_index`) — the relocation host call cannot
   extract suppressed panes, so this ordering is mandatory;
4. freshly spawned by the pipe itself (cold spawn, no cached event state) →
   show in place.

Branch selection SHALL be pure decision logic in `harpoon-core` (Constitution
I core/shim split); only the host calls live in the plugin shim.

#### Scenario: same-tab re-invoke after Esc-close
- **GIVEN** the plugin was shown and then hidden via Esc on tab T
- **WHEN** a `toggle` pipe message arrives while the client's active tab is T
- **THEN** the plugin SHALL show itself as a floating pane on tab T
- **AND** the client view SHALL remain on tab T

#### Scenario: cross-tab invoke relocates to the invoking tab
- **GIVEN** the plugin is hidden and parked on tab A
- **WHEN** a `toggle` pipe message arrives while the client's active tab is
  B (B ≠ A)
- **THEN** the plugin SHALL un-suppress itself BEFORE issuing the relocation
  host call
- **AND** the menu SHALL end as a floating pane on tab B with the client
  view on tab B

#### Scenario: correct under tab-id/position drift
- **GIVEN** at least one tab has been closed since the plugin's home tab was
  created (tab ids drifted above positions)
- **WHEN** a `toggle` pipe message arrives from any tab
- **THEN** the menu and the client view SHALL end on the invoking tab (never
  on a tab addressed by a stale tab id)

#### Scenario: visible toggle hides
- **GIVEN** the plugin is visible
- **WHEN** a `toggle` pipe message arrives
- **THEN** the plugin SHALL hide itself (`hide_self()`), preserving the
  mode-state-machine Close consolidation behavior

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

THE plugin SHALL establish the state driving a `toggle` decision —
its own pane's suppressed/visible state and the invoking client's focused
tab — via synchronous host queries at pipe-handling time
(`get_pane_info(PaneId::Plugin(own))`; `get_focused_pane_info()` returning
the stable tab ID, converted to a position via `get_tab_info(tab_id)`), and
SHALL NOT derive that state from cached `TabUpdate`/`PaneUpdate` events or
from `Event::Visible` (probe evidence 2026-07-11: event caches freeze while
the pane is suppressed, and zellij emits `Event::Visible` only to TILED
plugin panes — a floating plugin never receives it). Constitution IV: never
act on unverified host state.

#### Scenario: suppressed state queried, not assumed
- **GIVEN** the plugin has been hidden long enough for cached events to be
  stale (events do not flow to suppressed panes)
- **WHEN** a `toggle` pipe message arrives
- **THEN** the visibility decision SHALL come from a synchronous
  `get_pane_info` query of the plugin's own pane (fresh `is_suppressed`)

#### Scenario: relocation target queried, not cached
- **GIVEN** the client switched tabs while the plugin was hidden (cached
  `TabUpdate` still reports the pre-hide active tab)
- **WHEN** a `toggle` pipe message arrives from the new tab
- **THEN** the relocation target SHALL be the synchronously queried focused
  tab's position (`get_focused_pane_info` → `get_tab_info(...).position`),
  never the cached active tab

## MODIFIED Requirements

## REMOVED Requirements

## RENAMED Requirements

---

## Acceptance criterion quality checklist

| AC ID | Testable | Solution-free | Unambiguous | Consistent | Complete |
|---|---|---|---|---|---|
| pane-pipe-api.toggle-pipe-invocation | [x] | [x] | [x] | [x] | [x] |
| pane-pipe-api.toggle-state-sync-query-verified | [x] | [x] | [x] | [x] | [x] |
