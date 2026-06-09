//! `freeze_on_user_mutation` — compacts the sparse `state.panes` Vec to dense,
//! rewrites unresolved bookmark indices to `None`, and rebuilds
//! `store.pane_id_to_bookmark_idx`.
//!
//! Called as a pre-step from every mutation handler (`a`/`A`/`d`/`K`/`J`)
//! that has determined it WILL mutate. Pure no-op handlers (navigation, mode
//! transitions, slot-jumps) never trigger freeze.
//!
//! See `openspec/changes/add-filter-and-jump-modes/design.md`:
//! "Decision: state.panes is a sparse Vec<Option<Pane>>, made dense on first
//! user mutation" for the canonical algorithm and rationale.

use crate::bookmark::BookmarkStore;
use crate::dispatch::DispatchState;
use std::collections::HashMap;

/// Compact `state.panes` to a dense Vec, rewrite unresolved bookmark indices
/// to `None`, rebuild `store.pane_id_to_bookmark_idx`, and re-anchor
/// `state.selected` to the same pane by id.
///
/// **Algorithm** (verbatim from `design.md`):
///
/// 1. Capture `prev_selected_pane_id` from the slot at `state.selected` (may
///    be `None` if the selection is on a placeholder).
/// 2. Rewrite unresolved `Some(_)` bookmarks to `None`: collect indices where
///    `b.index.is_some() && !store.is_resolved(i)`, then null out each one.
///    (Two-phase to satisfy the borrow checker — see note below.)
/// 3. Compact panes: `state.panes.retain(|opt| opt.is_some())`.
/// 4. Rebuild `pane_id_to_bookmark_idx` and update each bookmark's `index` to
///    the new dense position. The rebuild reuses the EXISTING pane.id →
///    bk_idx associations (snapshotted before the clear), NOT a re-derivation
///    from `(tab_name, pane_title)`: titles are volatile (pi rewrites them),
///    so a title-based rebuild would drop the link for any pane whose live
///    title has drifted from its bookmark's stored title. The id↔bookmark
///    association is the authoritative, title-independent identity.
/// 5. Re-anchor `state.selected`: if `prev_selected_pane_id == Some(pid)`,
///    find `pid` in the post-compaction Vec; else fall back to `0`.
/// 6. Set `store.frozen = true`.
///
/// **Idempotency**: a second call on an already-dense, fully-resolved store is
/// a no-op in effect (panes unchanged, indices unchanged, `frozen` remains
/// `true`).
///
/// **Trigger sites** (from `design.md`):
/// - `a`/`A` (add): freeze BEFORE appending the new pane.
/// - `d` (delete): freeze only when `panes[selected].is_some()`.
/// - `K`/`J` (reorder): freeze before the swap.
/// Navigation, mode transitions, and slot-jumps do NOT freeze.
pub fn freeze_on_user_mutation(state: &mut DispatchState, store: &mut BookmarkStore) {
    // ── Step 1: remember which pane was selected (by id, survives compaction) ─
    let prev_selected_pane_id = state
        .panes
        .get(state.selected)
        .and_then(|opt| opt.as_ref())
        .map(|p| p.id);

    // ── Step 2: rewrite unresolved Some(_) bookmarks to None ─────────────────
    //
    // A bookmark is "unresolved" when no live-pane entry in
    // pane_id_to_bookmark_idx points to it — meaning its pane never appeared
    // in a PaneManifest this session (or was evicted).
    //
    // Collect first, mutate second, to avoid simultaneous borrows on `store`
    // (is_resolved() takes &self over the whole struct).
    let to_rewrite: Vec<usize> = store
        .bookmarks
        .iter()
        .enumerate()
        .filter(|(i, b)| b.index.is_some() && !store.is_resolved(*i))
        .map(|(i, _)| i)
        .collect();
    for i in to_rewrite {
        store.bookmarks[i].index = None;
    }

    // ── Step 3: compact panes (drop None placeholders) ────────────────────────
    state.panes.retain(|opt| opt.is_some());

    // ── Step 4: rebuild pane_id_to_bookmark_idx + update bookmark indices ─────
    //
    // Reuse the EXISTING pane.id → bk_idx associations (captured before the
    // clear) rather than re-deriving them from `(tab_name, pane_title)`.
    // Titles are volatile (pi rewrites them), so a title-based re-derivation
    // would silently drop the link for any pane whose live title has drifted
    // away from its bookmark's stored title. The id↔bookmark association is
    // the authoritative identity and is title-independent.
    let old_id_to_bk: HashMap<u32, usize> = store.pane_id_to_bookmark_idx.clone();
    store.pane_id_to_bookmark_idx.clear();

    for (new_idx, opt) in state.panes.iter().enumerate() {
        // Post-compaction every entry is Some; unwrap is safe.
        let pane = opt.as_ref().unwrap();
        if let Some(&bk_idx) = old_id_to_bk.get(&pane.id) {
            store.bookmarks[bk_idx].index = Some(new_idx as u16);
            store.pane_id_to_bookmark_idx.insert(pane.id, bk_idx);
        }
    }

    // ── Step 5: re-anchor selected to the same pane by id ────────────────────
    //
    // Post-compaction, pane positions may have shifted.  If the previously
    // selected slot was a placeholder (prev_selected_pane_id == None) or the
    // pane id can't be found (shouldn't happen), fall back to 0.
    state.selected = match prev_selected_pane_id {
        Some(pid) => state
            .panes
            .iter()
            .position(|opt| opt.as_ref().map(|p| p.id) == Some(pid))
            .unwrap_or(0),
        None => 0,
    };

    // ── Step 6: mark frozen ───────────────────────────────────────────────────
    store.frozen = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmark::PaneBookmark;
    use crate::pane::Pane;

    fn p(id: u32, tab: &str, title: &str) -> Pane {
        Pane {
            id,
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            tab_position: 0,
        }
    }

    fn bm(tab: &str, title: &str, index: Option<u16>) -> PaneBookmark {
        PaneBookmark {
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            index,
            id: None,
        }
    }

    // ── all_resolved_freeze_no_change ─────────────────────────────────────────

    /// Already-dense panes with all bookmarks resolved: freeze is a structural
    /// no-op (indices and panes unchanged).
    #[test]
    fn all_resolved_freeze_no_change() {
        let p0 = p(10, "work", "nvim");
        let p1 = p(11, "shell", "bash");
        let mut state = DispatchState {
            panes: vec![Some(p0.clone()), Some(p1.clone())],
            selected: 0,
            ..Default::default()
        };
        let mut store = BookmarkStore {
            bookmarks: vec![bm("work", "nvim", Some(0)), bm("shell", "bash", Some(1))],
            pane_id_to_bookmark_idx: [(10, 0), (11, 1)].into_iter().collect(),
            frozen: false,
        };

        freeze_on_user_mutation(&mut state, &mut store);

        assert_eq!(state.panes, vec![Some(p0), Some(p1)]);
        assert_eq!(store.bookmarks[0].index, Some(0));
        assert_eq!(store.bookmarks[1].index, Some(1));
        assert!(store.frozen);
    }

    // ── single_none_freeze_compacts ───────────────────────────────────────────

    /// One None placeholder in the middle: compacted to dense, unresolved
    /// bookmark index nulled, surviving bookmarks re-indexed.
    #[test]
    fn single_none_freeze_compacts() {
        let p0 = p(10, "work", "nvim");
        let p2 = p(12, "build", "cargo");
        let mut state = DispatchState {
            // Slot 1 is a placeholder (B1 never materialized).
            panes: vec![Some(p0.clone()), None, Some(p2.clone())],
            selected: 0,
            ..Default::default()
        };
        // B1 (shell/edit) is unresolved — no pane id maps to bk_idx 1.
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("shell", "edit", Some(1)), // unresolved placeholder
                bm("build", "cargo", Some(2)),
            ],
            pane_id_to_bookmark_idx: [(10, 0), (12, 2)].into_iter().collect(),
            frozen: false,
        };

        freeze_on_user_mutation(&mut state, &mut store);

        // Panes compacted: [Some(P0), Some(P2)]
        assert_eq!(state.panes.len(), 2);
        assert_eq!(state.panes[0].as_ref().unwrap().id, 10);
        assert_eq!(state.panes[1].as_ref().unwrap().id, 12);

        // B1 (unresolved) has its index cleared to None.
        assert_eq!(store.bookmarks[1].index, None);
        // B0 and B2 get updated dense indices.
        assert_eq!(store.bookmarks[0].index, Some(0));
        assert_eq!(store.bookmarks[2].index, Some(1));

        // Map rebuilt: bookmark-idx pointers unchanged (P0→0, P2→2).
        assert_eq!(store.pane_id_to_bookmark_idx.get(&10), Some(&0));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&12), Some(&2));

        assert!(store.frozen);
    }

    // ── selection tests ───────────────────────────────────────────────────────

    /// selected=0 (live pane P0): stays at 0 after freeze (P0 still at idx 0).
    #[test]
    fn selection_on_live_pre_freeze_stays_on_same_pane() {
        let p0 = p(10, "work", "nvim");
        let p2 = p(12, "build", "cargo");
        let mut state = DispatchState {
            panes: vec![Some(p0.clone()), None, Some(p2.clone())],
            selected: 0,
            ..Default::default()
        };
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("shell", "edit", Some(1)),
                bm("build", "cargo", Some(2)),
            ],
            pane_id_to_bookmark_idx: [(10, 0), (12, 2)].into_iter().collect(),
            frozen: false,
        };

        freeze_on_user_mutation(&mut state, &mut store);

        assert_eq!(state.selected, 0);
    }

    /// selected=1 (None placeholder): no pane id to anchor to → falls back to 0.
    #[test]
    fn selection_on_placeholder_falls_back_to_zero() {
        let p0 = p(10, "work", "nvim");
        let p2 = p(12, "build", "cargo");
        let mut state = DispatchState {
            panes: vec![Some(p0.clone()), None, Some(p2.clone())],
            selected: 1,
            ..Default::default()
        };
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("shell", "edit", Some(1)),
                bm("build", "cargo", Some(2)),
            ],
            pane_id_to_bookmark_idx: [(10, 0), (12, 2)].into_iter().collect(),
            frozen: false,
        };

        freeze_on_user_mutation(&mut state, &mut store);

        assert_eq!(state.selected, 0);
    }

    /// selected=2 (live pane P2 at old index 2): after compaction P2 moves to
    /// index 1 → selected re-anchors to 1.
    #[test]
    fn selection_on_later_live_re_anchors() {
        let p0 = p(10, "work", "nvim");
        let p2 = p(12, "build", "cargo");
        let mut state = DispatchState {
            panes: vec![Some(p0.clone()), None, Some(p2.clone())],
            selected: 2,
            ..Default::default()
        };
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("shell", "edit", Some(1)),
                bm("build", "cargo", Some(2)),
            ],
            pane_id_to_bookmark_idx: [(10, 0), (12, 2)].into_iter().collect(),
            frozen: false,
        };

        freeze_on_user_mutation(&mut state, &mut store);

        assert_eq!(state.selected, 1); // P2 moved from index 2 → 1
    }

    // ── duplicate_titles_distribute ───────────────────────────────────────────

    /// Two panes with identical (tab_name, pane_title): the "first unclaimed"
    /// logic distributes them across both bookmarks without collision.
    #[test]
    fn duplicate_titles_distribute() {
        let p0 = p(1, "work", "nvim");
        let p1 = p(2, "work", "nvim");
        let mut state = DispatchState {
            panes: vec![Some(p0.clone()), Some(p1.clone())],
            selected: 0,
            ..Default::default()
        };
        // Both bookmarks resolved (map has 1→0, 2→1).
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("work", "nvim", Some(1)),
            ],
            pane_id_to_bookmark_idx: [(1, 0), (2, 1)].into_iter().collect(),
            frozen: false,
        };

        freeze_on_user_mutation(&mut state, &mut store);

        // No None slots → compaction no-op; indices and map should be unchanged.
        assert_eq!(store.bookmarks[0].index, Some(0));
        assert_eq!(store.bookmarks[1].index, Some(1));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&1), Some(&0));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&2), Some(&1));
        assert!(store.frozen);
    }

    // ── frozen_flag_set ───────────────────────────────────────────────────────

    /// Freeze always sets `store.frozen = true` regardless of input.
    #[test]
    fn frozen_flag_set() {
        let mut state = DispatchState::default();
        let mut store = BookmarkStore::default();
        assert!(!store.frozen);
        freeze_on_user_mutation(&mut state, &mut store);
        assert!(store.frozen);
    }

    /// Map rebuild is id-based, not title-based: a live pane whose title has
    /// drifted away from its bookmark's stored title still keeps its
    /// pane↔bookmark link after freeze (the old title-based rebuild would have
    /// silently dropped it).
    #[test]
    fn freeze_rebuilds_map_by_id_despite_title_drift() {
        let live = p(10, "work", "NEW TITLE");
        let mut state = DispatchState {
            panes: vec![Some(live)],
            selected: 0,
            ..Default::default()
        };
        let mut store = BookmarkStore {
            // Bookmark's stored title differs from the live pane's title.
            bookmarks: vec![bm("work", "OLD TITLE", Some(0))],
            pane_id_to_bookmark_idx: [(10, 0)].into_iter().collect(),
            frozen: false,
        };

        freeze_on_user_mutation(&mut state, &mut store);

        // Link preserved by id; bookmark not nulled out.
        assert_eq!(store.pane_id_to_bookmark_idx.get(&10), Some(&0));
        assert_eq!(store.bookmarks[0].index, Some(0));
        assert_eq!(state.panes.len(), 1);
    }
}
