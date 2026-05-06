## ADDED Requirements

### Requirement: Shift selected pane up

In `Command` mode, `K` (shift-k) SHALL swap the pane at `selected` with the pane at `selected - 1` in `State.panes`. `selected` SHALL update to follow the moved pane (decrement by 1).

The handler SHALL be a no-op (returning `vec![]` — no `Save`, no `Render`, no `freeze`) when any of the following hold:
- `panes.len() < 2`
- `selected == 0`
- `selected >= panes.len()`
- `panes[selected].is_none()` (selection is on a placeholder)
- `panes[selected - 1].is_none()` (target slot is a placeholder — swap into a gap is rejected, preserves saved-position contract)

When pre-conditions are met, the handler SHALL:
1. Call `freeze_on_user_mutation(state, persistence)` first (compacts panes to dense; rebuilds `pane_id_to_bookmark_idx`; re-anchors `selected` to the same pane id).
2. Swap `state.panes[selected]` with `state.panes[selected - 1]` (post-freeze, both are guaranteed `Some`).
3. Swap the corresponding bookmarks' `index` fields in `Persistence::bookmarks` using `pane_id_to_bookmark_idx`.
4. Decrement `selected`.
5. Return `[Effect::Save, Effect::Render]`.

#### Scenario: Shift up swaps with previous and follows
- **GIVEN** mode is `Command` and `selected = 2`
- **AND** `panes[1].id = X` and `panes[2].id = Y`
- **WHEN** the user presses `K`
- **THEN** `panes[1].id == Y` and `panes[2].id == X`
- **AND** `selected == 1`
- **AND** persistence is invoked

#### Scenario: Shift up at top is a no-op
- **GIVEN** mode is `Command` and `selected = 0`
- **WHEN** the user presses `K`
- **THEN** `State.panes` is unchanged
- **AND** `selected == 0`
- **AND** no persistence write occurs (no `Save` effect)

#### Scenario: Shift up on empty list is a no-op
- **GIVEN** mode is `Command` and `panes.is_empty()`
- **WHEN** the user presses `K`
- **THEN** no panic occurs
- **AND** no persistence write occurs

#### Scenario: Shift up on single-element list is a no-op
- **GIVEN** mode is `Command` and `panes.len() == 1`
- **WHEN** the user presses `K`
- **THEN** `State.panes` is unchanged
- **AND** no persistence write occurs

### Requirement: Shift selected pane down

In `Command` mode, `J` (shift-j) SHALL swap the pane at `selected` with the pane at `selected + 1`. `selected` SHALL update to follow the moved pane (increment by 1).

The handler SHALL be a no-op (returning `vec![]`) when any of the following hold:
- `panes.len() < 2`
- `selected >= panes.len() - 1`
- `panes[selected].is_none()` (selection is on a placeholder)
- `panes[selected + 1].is_none()` (target slot is a placeholder)

When pre-conditions are met, the handler SHALL freeze, swap, swap matching bookmark indices, increment `selected`, and return `[Effect::Save, Effect::Render]` (same flow as `K`, mirrored direction).

#### Scenario: Shift down swaps with next and follows
- **GIVEN** mode is `Command` and `selected = 2`
- **AND** `panes[2].id = X` and `panes[3].id = Y`
- **WHEN** the user presses `J`
- **THEN** `panes[2].id == Y` and `panes[3].id == X`
- **AND** `selected == 3`
- **AND** persistence is invoked

#### Scenario: Shift down at bottom is a no-op
- **GIVEN** mode is `Command` and `selected == panes.len() - 1`
- **WHEN** the user presses `J`
- **THEN** `State.panes` is unchanged
- **AND** `selected` is unchanged
- **AND** no persistence write occurs

#### Scenario: Shift down on empty list is a no-op
- **GIVEN** mode is `Command` and `panes.is_empty()`
- **WHEN** the user presses `J`
- **THEN** no panic occurs
- **AND** no persistence write occurs

#### Scenario: Shift down on single-element list is a no-op
- **GIVEN** mode is `Command` and `panes.len() == 1`
- **WHEN** the user presses `J`
- **THEN** `State.panes` is unchanged
- **AND** no persistence write occurs

### Requirement: Reorder remaps slot mapping

Because slot mapping (`1-9`, `a-z`) is a pure function of `State.panes` index, any reorder operation SHALL implicitly remap which pane responds to which slot key without any additional logic.

#### Scenario: Shifting a pane changes its slot
- **GIVEN** mode is `Command`, `selected = 2`, and `panes[2].id = X`
- **AND** slot `3` resolves to `panes[2]` before the shift
- **WHEN** the user presses `K`
- **THEN** slot `3` now resolves to a different pane (the one that was at index 2 before)
- **AND** slot `2` now resolves to the pane with id `X`

### Requirement: Reorder persists immediately

Each successful reorder operation SHALL emit `Effect::Save` from the dispatch handler. The plugin shim translates `Effect::Save` into `persistence.save_if_changed()` (no-arg form; `Persistence::bookmarks` is the canonical disk shape, kept in sync inline by the handler).

#### Scenario: Reorder writes to disk
- **GIVEN** mode is `Command` and `selected = 2`
- **AND** persistence is configured with a `session_name`
- **WHEN** the user presses `K`
- **THEN** the handler returns `[Effect::Save, Effect::Render]`
- **AND** the shim calls `persistence.save_if_changed()` which writes the v2 envelope to the session file at `${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon/<session>.json`
- **AND** the persisted file's `bookmarks` Vec reflects the new order via updated `index` fields

### Requirement: Manual reorder is canonical and survives session reload

Once the user has performed any `K`/`J` reorder, the manual `State.panes` order SHALL be the authoritative slot ordering. The persistence schema SHALL be upgraded so that each saved bookmark records its Vec position at save time (`PaneBookmark.index: Option<u16>`, where `Some(i)` = saved-position placement and `None` = append-on-resolve for post-freeze entries). Code paths that previously sorted `State.panes` by `tab_info.position` (specifically the existing `sort_panes()` helper and its callers) SHALL be modified so that they never reorder existing entries. Specifically:

- The `a` (add focused) command-mode path SHALL append the new pane (as `Some(p)`) to the end of `State.panes` without reordering existing entries.
- The `A` (add all) command-mode path SHALL append new panes in **deterministic order**: tab `position` ASC, then within each tab `PaneInfo.id` ASC. `PaneInfo.id` is monotonically assigned by zellij, so this ordering is stable across runs and across zellij versions, unlike the host-implementation-defined `PaneManifest.panes[tab_position]` Vec order.
- The restore resolution path (`resolve_restore_round`) SHALL pre-size `State.panes` via `resize(max_saved_idx + 1, None)` and place each resolved pane at its saved index via `state.panes[i] = Some(p)` (no shifting, no clamping — the Vec is sized to fit). It SHALL NOT call `sort_panes()` afterward. Unresolved bookmarks leave their saved-index slots as `None` (placeholders).
- Any remaining "sort by tab position" behavior, if retained at all, SHALL apply only the very first time a fresh session populates `State.panes` with no persisted state and no prior reorder.

#### Scenario: Reorder survives single-round session reload
- **GIVEN** mode is `Command`, `panes = [P0, P1, P2]`, persistence configured
- **WHEN** the user presses `K` while `selected = 2` (resulting order `[P0, P2, P1]`)
- **AND** the zellij session is closed and reopened
- **AND** all three panes are visible in the first `update_panes` round
- **THEN** `State.panes` is `[P0, P2, P1]` in that order
- **AND** slot `2` resolves to `P2` and slot `3` resolves to `P1`

#### Scenario: Reorder survives staggered (multi-round) restore
- **GIVEN** persisted order is `[P0, P1, P2]` (saved indices `0, 1, 2`)
- **AND** the zellij session reopens with only `P0` and `P2` visible in the first `update_panes` round
- **WHEN** `resolve_restore_round` places `P0` at `state.panes[0] = Some(P0)` and `P2` at `state.panes[2] = Some(P2)` (with `state.panes[1] = None` placeholder)
- **AND** a later `update_panes` round resolves `P1`, setting `state.panes[1] = Some(P1)`
- **THEN** `State.panes` ends up as `[Some(P0), Some(P1), Some(P2)]` in saved index order
- **AND** during the partial-restore window between rounds, slot `2` is a placeholder (rendering `"2  ?  (resolving)"`) and pressing `2` returns `vec![]` (no-op)

#### Scenario: Add focused pane preserves existing manual order
- **GIVEN** mode is `Command`, `panes = [P0, P2, P1]` (after a prior reorder)
- **AND** a new pane `P3` is focused
- **WHEN** the user presses `a`
- **THEN** `State.panes` becomes `[P0, P2, P1, P3]`
- **AND** the relative order of `P0`, `P2`, `P1` is preserved

#### Scenario: Add all preserves existing order and uses deterministic per-tab ordering
- **GIVEN** mode is `Command`, `panes = [P0, P2, P1]`, and tabs contain new panes `P4` (tab position 0, `PaneInfo.id = 7`), `P5` (tab position 1, `PaneInfo.id = 5`)
- **WHEN** the user presses `A`
- **THEN** the existing entries `[P0, P2, P1]` retain their relative order at the head of the list
- **AND** `P4` is appended before `P5` (because tab position 0 < 1)
- **AND** repeating the test produces the same result on every run

#### Scenario: Add all sorts within a tab by PaneInfo.id
- **GIVEN** mode is `Command`, `panes = []`, and tab position 0 contains two new panes with `PaneInfo.id = 9` and `PaneInfo.id = 4`
- **WHEN** the user presses `A`
- **THEN** the pane with `id = 4` is appended before the pane with `id = 9`

### Requirement: Persistence schema v2 envelope with single bookmarks Vec

The on-disk bookmark schema SHALL be a top-level JSON envelope `{ "version": 2, "bookmarks": [...] }` where each entry is a `PaneBookmark` with `index: Option<u16>`. The semantics of `index` are:

- `Some(i)`: place this pane at saved Vec position `i` on next reload. The restore loop pre-sizes `State.panes` (which is `Vec<Option<Pane>>`) so that index `i` exists; resolution writes `state.panes[i] = Some(p)` without shifting other entries. Unresolved saved-index slots remain `None` and render as placeholders.
- `None`: append this pane to the end of `State.panes` when it next resolves (used for post-freeze append-on-resolve entries). On resolution, the bookmark's `index` field is rewritten to `Some(new_dense_position)`.

A single Vec carries both materialized panes and post-freeze pending entries; there is no separate `materialized` vs `pending` array.

#### Scenario: Save writes v2 envelope with mixed indices
- **GIVEN** `State.panes = [P0, P1, P2]` with persistence configured
- **AND** the user previously froze restore with `P3` unresolved (now in `bookmarks` with `index = None`)
- **WHEN** `save_if_changed()` is invoked (which calls `save_to_disk()` internally when the canonical bookmarks Vec differs from `last_saved_state`)
- **THEN** the on-disk JSON is `{ "version": 2, "bookmarks": [ {tab_name, pane_title, index: 0}, {tab_name, pane_title, index: 1}, {tab_name, pane_title, index: 2}, {tab_name, pane_title, index: null} ] }`
- **AND** the entries for P0/P1/P2 have `index = Some(i)`
- **AND** the entry for P3 has `index = None`

#### Scenario: Load reads v1 (legacy) bookmarks without envelope
- **GIVEN** the on-disk JSON is a legacy v1 bare array `[ {tab_name, pane_title}, ... ]`
- **WHEN** the plugin loads
- **THEN** v2 envelope deserialization fails and the loader falls back to v1 array format
- **AND** indices are assigned in array order (`bookmarks[i].index = Some(i as u16)`)
- **AND** the next save writes the v2 envelope format

#### Scenario: V1 binary reading v2 file fails to load and starts empty
- **GIVEN** the on-disk JSON is the v2 envelope and the running binary is v1 (no envelope support)
- **WHEN** v1 attempts to load
- **THEN** deserialization fails (envelope is not a bare `Vec<PaneBookmark>`)
- **AND** v1 logs `LoadFromDiskFailed` and starts with an empty bookmark set
- **AND** the v2 file is NOT mutated by v1's load attempt

### Requirement: Non-unique identity panes are best-effort across reload

When two or more panes have identical `(tab_name, pane_title)` anywhere in the session (whether in the same tab or different tabs), persistence cannot distinguish them across reloads. Their relative order in the restored `State.panes` is implementation-defined and MAY differ from the saved order.

This limitation is broader than "duplicate within tab": tab names themselves may not be unique session-wide (zellij allows multiple tabs with the same name), so two `("work", "nvim")` bookmarks in two different `"work"`-named tabs collide on identity.

Within a single session (no reload), reorder of non-unique-identity panes SHALL persist to disk. The `Persistence::has_changed` comparison SHALL be index-aware (compares the full envelope shape including `index` fields), so a swap of two such bookmarks triggers a save.

#### Scenario: Duplicate-title bookmarks are best-effort across reload
- **GIVEN** the user pinned two panes both with `tab_name = "work"` and `pane_title = "nvim"`
- **AND** the user manually reordered them via `K`/`J`
- **WHEN** the session is reloaded
- **THEN** the resolved order between the two duplicates is implementation-defined
- **AND** the documentation explicitly notes this limitation

#### Scenario: In-session swap of duplicate-titled panes persists
- **GIVEN** `panes = [Some(P0), Some(P1)]` where both have identical `(tab_name, pane_title)` but different `id`s
- **AND** persistence is configured
- **WHEN** the user presses `K` while `selected = 1` (swap to `[Some(P1), Some(P0)]`); identity is tracked via `pane_id_to_bookmark_idx`, so the correct bookmark indices are swapped
- **THEN** `Persistence::has_changed()` (no-arg) returns `true` because the canonical `Persistence::bookmarks` Vec now has the two duplicate-titled bookmarks at swapped `index` fields
- **AND** `save_if_changed()` is invoked
- **AND** the on-disk JSON reflects the new order via the `index` field

### Requirement: Restore freeze on user mutation rewrites unresolved indices to None

When the plugin is loading and bookmarks are being restored across multiple `update_panes` rounds (some entries in `Persistence::bookmarks` have `index = Some(_)` and have NOT yet resolved into `State.panes`), any user mutation key (`a`, `A`, `d`, `K`, `J`) SHALL freeze restore. The freeze SHALL rewrite EVERY unresolved bookmark's `index` from `Some(i)` to `None`, preserving the bookmark in `Persistence::bookmarks` but switching its semantics from saved-position placement to append-on-resolve.

After the freeze, on every subsequent `update_panes` round, any bookmark with `index = None` whose `(tab_name, pane_title)` becomes resolvable SHALL be appended to `State.panes` (not inserted at any saved index).

While any bookmark still has `index = Some(_)` AND has not resolved (the unfrozen restore phase), `State.panes` SHALL be sparse — resolved bookmarks materialize as `Some(p)` at their saved index, unresolved bookmarks render as placeholders via `state.panes[i] = None`. Newly-visible panes that are NOT in `Persistence::bookmarks` SHALL NOT be auto-appended during this phase; the user controls additions via `a`/`A`.

#### Scenario: User reorders during partial restore rewrites unresolved indices
- **GIVEN** persisted bookmarks are `[B0(index=0), B1(index=1), B2(index=2)]`
- **AND** the session reopens; on first `update_panes` round only `B0` and `B2` resolve
- **AND** `state.panes = [Some(P0), None, Some(P2)]` (sparse; index 1 is a placeholder)
- **WHEN** the user attempts K with `selected = 2`
- **THEN** the K pre-condition `panes[selected - 1].is_some()` fails (panes[1] is None), so K returns `vec![]` (no freeze, no swap)
- **AND** the user moves `selected = 0`, then presses `J` (reorder down)
- **AND** the J pre-condition `panes[selected + 1].is_some()` fails (panes[1] is None), so J returns `vec![]`
- **AND** the user presses `d` with `selected = 0` (Live target P0)
- **THEN** `freeze_on_user_mutation` fires:
  - Unresolved `B1(index=1)` is rewritten to `B1(index=None)`
  - `state.panes` is compacted from `[Some(P0), None, Some(P2)]` to `[Some(P0), Some(P2)]`
  - `pane_id_to_bookmark_idx` rebuilt; `B0`'s saved index updates to `0`, `B2`'s saved index updates to `1`
- **AND** the d handler then removes `state.panes[0]`, resulting in `state.panes = [Some(P2)]`
- **AND** when `B1` later resolves, the corresponding pane is appended (because `index = None`): `state.panes = [Some(P2), Some(P1)]`

#### Scenario: Save during partial-restore-after-mutation persists None for unresolved
- **GIVEN** the freeze scenario above immediately after `K`, with `B1.index = None`
- **AND** the user immediately presses `a` to add a new focused pane `P3` (triggering `Effect::Save`)
- **WHEN** `save_if_changed()` runs (the canonical no-arg entrypoint that compares `Persistence::bookmarks` against `last_saved_state` and calls `save_to_disk()` if changed)
- **THEN** the on-disk JSON envelope `bookmarks` contains four entries:
  - `B2 { tab_name, pane_title, index: Some(0) }`
  - `B0 { tab_name, pane_title, index: Some(1) }`
  - `B3 { tab_name, pane_title, index: Some(2) }` (the newly-added P3)
  - `B1 { tab_name, pane_title, index: None }` (still pending late-resolve)
- **AND** on the NEXT session reload, `B1` enters resolution with `index = None` semantics and appends when its pane becomes visible
- **AND** is NOT placed at its old saved position

#### Scenario: Non-bookmark pane visible during partial restore is NOT auto-appended
- **GIVEN** persisted bookmarks are `[B0(index=0), B1(index=1)]`
- **AND** session reopens; `B0` resolved, `B1` unresolved (saved-index restore still in progress)
- **AND** a brand-new pane `Pnew` (never previously bookmarked) is visible in the same `update_panes` round
- **WHEN** `update_panes` materializes `State.panes`
- **THEN** `State.panes` is `[P0]` (only the resolved bookmarked entry)
- **AND** `Pnew` is NOT auto-appended; the user must press `a` or `A` to add it

#### Scenario: No user mutation during restore preserves saved-index placement
- **GIVEN** persisted bookmarks are `[B0(index=0), B1(index=1), B2(index=2)]`
- **AND** the session reopens with P0 and P2 visible in round 1, P1 in round 2
- **AND** the user does NOT press any mutation keys between rounds
- **WHEN** P1 resolves in round 2
- **THEN** `State.panes` is `[P0, P1, P2]` in saved-index order
- **AND** is NOT `[P0, P2, P1]`

### Requirement: Reorder is command-mode only

`K` and `J` SHALL only trigger reorder when mode is `Command`. In `Filter` mode they are query characters; in `Jump` mode they are slot keys (or ignored if no such slot).

#### Scenario: K does not reorder in filter mode
- **GIVEN** mode is `Filter` and `selected = 2`
- **WHEN** the user presses `K`
- **THEN** query gains the character `K`
- **AND** `State.panes` is unchanged

#### Scenario: J does not reorder in jump mode
- **GIVEN** mode is `Jump` and `selected = 2`
- **WHEN** the user presses `J` (uppercase)
- **THEN** `State.panes` is unchanged
- **AND** the keypress is ignored (slot keys are lowercase `a-z` only)
- **AND** mode remains `Jump`
