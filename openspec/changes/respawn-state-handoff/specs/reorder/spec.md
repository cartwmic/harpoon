# Capability: reorder (delta — respawn-state-handoff)

## ADDED Requirements

### Requirement: Destructive Save Guard

THE plugin SHALL suppress any save that would remove bookmarks present in
the last persisted state (a shrinking save) WHILE the instance has NOT yet
observed BOTH a resolved disk load (success or explicit failure) AND a
full pane manifest. Additive or reordering saves SHALL remain allowed
during this window. Once both conditions have been observed, normal
reconciliation pruning and its saves apply unchanged. The
guard decision SHALL be pure logic in `harpoon-core` with native tests
(Constitution I).

#### Scenario: early partial manifest cannot shrink the disk file
- **GIVEN** a freshly spawned instance whose disk load has not yet resolved
  (or whose first pane manifest is partial)
- **WHEN** reconciliation computes a bookmark list smaller than the last
  persisted one and a save is attempted
- **THEN** the shrinking save SHALL be suppressed and the on-disk bookmark
  file SHALL retain all previously persisted bookmarks

#### Scenario: additive save during the bootstrap window persists
- **GIVEN** the same early window (disk load or full manifest not yet
  observed)
- **WHEN** the user adds a bookmark and a save is attempted
- **THEN** the save SHALL proceed (the guard blocks only shrinking saves)

#### Scenario: pruning resumes after both conditions observed
- **GIVEN** the instance has observed a resolved disk load AND a full pane
  manifest
- **WHEN** reconciliation drops a bookmark whose pane is genuinely gone and
  a save runs
- **THEN** the save SHALL proceed and the on-disk file SHALL reflect the
  pruned list (the guard never becomes a permanent no-shrink rule)

### Requirement: Restore Identity Tracks Live Panes

THE plugin SHALL keep each resolved bookmark's persisted identity current
with the live pane it is bound to: bookmark resolution SHALL match by
stable pane id first, falling back to `(tab_name, pane_title)` only when no
persisted id matches a visible pane; and WHEN a resolved pane's observed
`tab_name` or `pane_title` changes, the bookmark's persisted identity
fields SHALL be refreshed (and persisted via the normal save path) so the
on-disk fallback identity is the most recently observed one — reducing
restore dependence on stale volatile titles after pane ids reset (session
restart). The respawn hand-off payload SHALL carry resolved pane ids so a
respawned successor resolves bookmarks by id without any title matching.

This requirement SHALL NOT regress the existing restore semantics:
restore-freeze on user mutation, placeholder slots for unresolved
saved-index bookmarks, and best-effort ordering for non-unique-identity
panes all apply unchanged.

#### Scenario: respawned successor resolves by id, ignoring title drift
- **GIVEN** a bookmark resolved to pane id 7 whose title has since been
  rewritten by the pane's program
- **WHEN** the respawn hand-off delivers the store to a successor in the
  same zellij session
- **THEN** the successor SHALL resolve the bookmark to pane id 7 by id
  match alone (title mismatch is irrelevant)

#### Scenario: title drift is re-persisted while the pane is resolved
- **GIVEN** a resolved bookmark whose live pane's title changes from
  "nvim" to "pi — fixing restore"
- **WHEN** the plugin observes the change in a pane manifest update
- **THEN** the bookmark's persisted `pane_title` SHALL be refreshed to the
  new title via the normal save path

#### Scenario: cross-restart fallback uses the freshest identity
- **GIVEN** a zellij session restart reset all pane ids (no persisted id
  matches any visible pane)
- **WHEN** restore runs and falls back to `(tab_name, pane_title)` matching
- **THEN** matching SHALL use the most recently persisted identity (the
  title-drift-refreshed values), and an unmatched bookmark SHALL follow the
  existing unresolved semantics (placeholder slot or append-on-resolve) —
  never a speculative fuzzy match

---

## Acceptance criterion quality checklist

| AC ID | Testable | Solution-free | Unambiguous | Consistent | Complete |
|---|---|---|---|---|---|
| reorder.destructive-save-guard | [x] | [x] | [x] (window = both conditions observed) | [x] (empty-store first-save guard unchanged) | [x] (suppress + allow + resume) |
| reorder.restore-identity-tracks-live-panes | [x] | [x] | [x] | [x] (explicit non-regression clause vs freeze/placeholder/best-effort) | [x] (in-session id, drift refresh, cross-restart fallback) |
