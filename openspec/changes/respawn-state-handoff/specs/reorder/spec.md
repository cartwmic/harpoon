# Capability: reorder (delta — respawn-state-handoff)

## ADDED Requirements

### Requirement: Destructive Save Guard

THE plugin SHALL suppress any save that would remove bookmarks present in
the last persisted state (a shrinking save) WHILE the instance has NOT yet
observed BOTH a resolved disk load (success or explicit failure) AND a full
pane manifest, and SHALL preserve those bookmarks in memory during
partial-manifest reconciliation (so a later ready-state save cannot flush a
previously pruned list). "Full pane manifest" means the PaneUpdate snapshot
contains an entry for every tab position currently known from TabUpdate —
the strongest completeness condition exposed by zellij 0.44.3.

Additive or reordering mutations SHALL remain allowed during this window.
When a persisted baseline is already known (adopted or disk-loaded), their
non-shrinking save SHALL proceed immediately. When no baseline is known yet,
the mutation SHALL remain in memory and its disk flush SHALL be queued until
the disk load resolves and reconciles — fail-closed deferral avoids replacing
an unknown fuller disk file with a partial candidate. Once both readiness
conditions have been observed, normal reconciliation pruning and its saves
apply unchanged, including deferred removal of a live pane that disappeared
while pruning was guarded OR a same-session hand-off id absent from the
successor's first full manifest (neither SHALL become a permanent unresolved
ghost).
Guard/readiness/save-policy/deferred-prune decisions SHALL be pure logic in
`harpoon-core` with native tests (Constitution I).

#### Scenario: early partial manifest cannot shrink the disk file
- **GIVEN** a freshly spawned instance whose disk load has not yet resolved
  (or whose first pane manifest is partial)
- **WHEN** reconciliation computes a bookmark list smaller than the last
  persisted one and a save is attempted
- **THEN** the shrinking save SHALL be suppressed and the on-disk bookmark
  file SHALL retain all previously persisted bookmarks

#### Scenario: additive mutation with known baseline persists immediately
- **GIVEN** the same early window (disk load or full manifest not yet
  observed) AND a persisted baseline is known from bootstrap or disk
- **WHEN** the user adds a bookmark and a save is attempted
- **THEN** the non-shrinking save SHALL proceed (the guard blocks only
  shrinking saves)

#### Scenario: additive mutation before baseline is queued safely
- **GIVEN** the disk load is unresolved, no bootstrap was adopted, and the
  persisted baseline is unknown
- **WHEN** the user adds a bookmark
- **THEN** the in-memory mutation SHALL proceed immediately, its full-file
  disk write SHALL be deferred, and the mutation SHALL be preserved through
  late disk reconciliation
- **AND** once the disk baseline resolves the merged non-shrinking state
  SHALL be flushed through the normal save path

#### Scenario: pruning resumes after both conditions observed
- **GIVEN** the instance has observed a resolved disk load AND a full pane
  manifest
- **WHEN** reconciliation drops a bookmark whose pane is genuinely gone and
  a save runs
- **THEN** the save SHALL proceed and the on-disk file SHALL reflect the
  pruned list (the guard never becomes a permanent no-shrink rule)

### Requirement: Restore Identity Tracks Live Panes

THE plugin SHALL keep each resolved bookmark's persisted identity current
with the live pane it is bound to. Same-session state SHALL match by stable
pane id first, falling back to `(tab_name, pane_title)` only when no trusted
id matches a visible pane; and WHEN a resolved pane's observed `tab_name` or
`pane_title` changes, the bookmark's persisted identity fields SHALL be
refreshed (and persisted via the normal save path) so the on-disk fallback
identity is the most recently observed one. Pane ids parsed from disk SHALL
be cleared before restore/merge/shrink comparison because disk outlives a
zellij session generation and ids may be reassigned to unrelated panes. The
respawn hand-off payload SHALL retain resolved pane ids because predecessor
and successor share one generation, so the successor resolves by id without
title matching.

This requirement SHALL NOT regress the existing restore semantics:
restore-freeze on user mutation, placeholder rows for unresolved bookmarks
(including valid post-freeze `index=None` rows appended after indexed slots),
and best-effort ordering for non-unique-identity panes all apply unchanged.
Merge/shrink/restore comparison SHALL preserve duplicate multiplicity across
staggered rounds: trusted differing ids are distinct, every visible exact-id
claim SHALL be reserved globally before any fallback claim, and one fallback
row or one live pane SHALL NOT satisfy two persisted duplicate rows.

#### Scenario: respawned successor resolves by id, ignoring title drift
- **GIVEN** a bookmark resolved to pane id 7 whose title has since been
  rewritten by the pane's program
- **WHEN** the respawn hand-off delivers the store to a successor in the
  same zellij session
- **THEN** the successor SHALL resolve the bookmark to pane id 7 by id
  match alone (title mismatch is irrelevant)

#### Scenario: cold disk restore rejects a reused pane id
- **GIVEN** disk bookmark `(id=7, tab=work, title=nvim)` survived a session
  restart and unrelated live pane `(id=7, tab=shell, title=bash)` now exists
- **WHEN** the cold successor parses and restores the disk file
- **THEN** it SHALL clear the generation-unverified persisted id before
  resolution and SHALL NOT bind the bookmark to the unrelated pane
- **AND** fallback identity SHALL remain available to find the intended pane

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
