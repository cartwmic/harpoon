//! Restore-resolution algorithm.
//!
//! Each `update_panes` round in the plugin shim calls [`resolve_restore_round`]
//! to materialize bookmarks into live panes as their `(tab_name, pane_title)`
//! identity becomes visible in the zellij PaneManifest.
//!
//! See `design.md` "Decision: Restore resolution algorithm (sparse Vec,
//! concrete)" — this module is the canonical implementation referenced
//! there.

use std::collections::HashSet;

use crate::bookmark::BookmarkStore;
use crate::pane::Pane;

/// One pane visible in the zellij PaneManifest, projected to host-agnostic
/// shape. The plugin shim builds a `Vec<VisiblePane>` from
/// `PaneManifest` + `TabInfo` and passes it to `resolve_restore_round`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisiblePane {
    pub id: u32,
    pub tab_name: String,
    pub pane_title: String,
    pub tab_position: u32,
}

impl VisiblePane {
    pub fn into_pane(self) -> Pane {
        Pane {
            id: self.id,
            tab_name: self.tab_name,
            pane_title: self.pane_title,
            tab_position: self.tab_position,
        }
    }
}

/// Resolve as many bookmarks as possible against the currently-visible
/// panes. Mutates `state_panes` (the sparse `Vec<Option<Pane>>`) and
/// `store` in lockstep:
///
/// 1. Pre-size `state_panes` so saved indices have slots: resize with `None`
///    up to `max_saved_idx + 1`.
/// 2. For each unresolved bookmark, match a visible pane by **stable id
///    first** (`bookmark.id == visible.id`), falling back to
///    `(tab_name, pane_title)` only when the bookmark has no id or its id is
///    not currently visible (first-match-wins; `consumed` set tracks
///    distribution for duplicate bookmarks). On match, the bookmark's
///    `id`/`tab_name`/`pane_title` are refreshed from the live pane:
///    - If `bookmark.index = Some(i)`: write `state_panes[i] = Some(p)`.
///    - If `bookmark.index = None`: append `Some(p)` and rewrite the
///      bookmark's `index` to `Some(new_dense_position)`.
///    - Insert `pane.id → bookmark_idx` in `pane_id_to_bookmark_idx`.
/// 3. Bookmarks not yet visible remain unchanged (they'll be tried again
///    on the next call).
///
/// **Non-bookmarked visible panes are NOT auto-added** — only bookmarked
/// panes are restored. The user controls additions via `a`/`A` in command
/// mode.
pub fn resolve_restore_round(
    store: &mut BookmarkStore,
    state_panes: &mut Vec<Option<Pane>>,
    visible: &[VisiblePane],
) {
    // ── Step 1: pre-size panes ──────────────────────────────────────────────
    let max_saved_idx = store
        .bookmarks
        .iter()
        .filter_map(|b| b.index)
        .max()
        .map(|x| x as usize);
    if let Some(max) = max_saved_idx {
        if state_panes.len() < max + 1 {
            state_panes.resize(max + 1, None);
        }
    }

    // ── Step 2: walk bookmarks, claim visible panes ──────────────────────────
    let mut consumed_visible_ids: HashSet<u32> = HashSet::new();

    // Defer mutations that need bookmark index rewrites until after the
    // bookmark borrow is released.
    let mut to_append: Vec<(usize /* bookmark idx */, Pane /* the resolved pane */)> = Vec::new();
    let mut to_place: Vec<(usize /* bookmark idx */, usize /* slot */, Pane)> = Vec::new();

    for (bk_idx, b) in store.bookmarks.iter().enumerate() {
        // Skip already-resolved bookmarks (their pane id is in the map).
        if store.is_resolved(bk_idx) {
            continue;
        }

        // ── Match by stable id FIRST (title-independent) ────────────────────
        // Pane titles drift (pi rewrites them), so a saved (tab,title) may no
        // longer match the live pane. The pane id is stable for the session,
        // so prefer it. Fall back to (tab_name, pane_title) only when the
        // bookmark has no id yet (older on-disk file) or its id is no longer
        // visible (cross-session restore: ids were reassigned).
        let matched = b
            .id
            .and_then(|bid| {
                visible
                    .iter()
                    .find(|v| v.id == bid && !consumed_visible_ids.contains(&v.id))
            })
            .or_else(|| {
                visible.iter().find(|v| {
                    v.tab_name == b.tab_name
                        && v.pane_title == b.pane_title
                        && !consumed_visible_ids.contains(&v.id)
                })
            });

        let Some(v) = matched else {
            continue; // not yet visible; try next round
        };
        consumed_visible_ids.insert(v.id);

        let p = Pane {
            id: v.id,
            tab_name: v.tab_name.clone(),
            pane_title: v.pane_title.clone(),
            tab_position: v.tab_position,
        };

        match b.index {
            Some(i) => to_place.push((bk_idx, i as usize, p)),
            None => to_append.push((bk_idx, p)),
        }
    }

    // ── Step 2 application: place / append, update bookmarks + map ──────────
    // On every resolve, refresh the bookmark's stored id + identity from the
    // live pane so disk stays current (the id we just matched, and the live
    // tab_name/pane_title even if they drifted since the bookmark was saved).
    for (bk_idx, slot, p) in to_place {
        let pid = p.id;
        store.bookmarks[bk_idx].id = Some(pid);
        store.bookmarks[bk_idx].tab_name = p.tab_name.clone();
        store.bookmarks[bk_idx].pane_title = p.pane_title.clone();
        // The slot was pre-sized; just write into it.
        state_panes[slot] = Some(p);
        store.pane_id_to_bookmark_idx.insert(pid, bk_idx);
    }

    for (bk_idx, p) in to_append {
        let new_idx = state_panes.len();
        store.bookmarks[bk_idx].index = Some(new_idx as u16);
        store.bookmarks[bk_idx].id = Some(p.id);
        store.bookmarks[bk_idx].tab_name = p.tab_name.clone();
        store.bookmarks[bk_idx].pane_title = p.pane_title.clone();
        store.pane_id_to_bookmark_idx.insert(p.id, bk_idx);
        state_panes.push(Some(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmark::PaneBookmark;

    fn vp(id: u32, tab: &str, title: &str) -> VisiblePane {
        VisiblePane {
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

    // ── Single-round full restore ──────────────────────────────────────────

    #[test]
    fn single_round_resolves_all_bookmarks_at_saved_indices() {
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("shell", "edit", Some(1)),
                bm("build", "cargo", Some(2)),
            ],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        let visible = vec![
            vp(10, "work", "nvim"),
            vp(11, "shell", "edit"),
            vp(12, "build", "cargo"),
        ];

        resolve_restore_round(&mut store, &mut panes, &visible);

        assert_eq!(panes.len(), 3);
        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert_eq!(panes[1].as_ref().unwrap().id, 11);
        assert_eq!(panes[2].as_ref().unwrap().id, 12);
        assert_eq!(store.pane_id_to_bookmark_idx.get(&10), Some(&0));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&11), Some(&1));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&12), Some(&2));
    }

    // ── Staggered restore (multi-round) ─────────────────────────────────────

    #[test]
    fn staggered_restore_preserves_saved_positions() {
        // B0 at idx 0, B1 at idx 1, B2 at idx 2.
        // Round 1: only B0 and B2 visible.
        // Round 2: B1 becomes visible.
        // Final: panes = [Some(P0), Some(P1), Some(P2)] in saved-index order.
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("shell", "edit", Some(1)),
                bm("build", "cargo", Some(2)),
            ],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();

        // Round 1: B0 + B2 visible, B1 not yet.
        let r1 = vec![vp(10, "work", "nvim"), vp(12, "build", "cargo")];
        resolve_restore_round(&mut store, &mut panes, &r1);
        assert_eq!(panes.len(), 3);
        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert!(panes[1].is_none(), "B1 unresolved → placeholder slot");
        assert_eq!(panes[2].as_ref().unwrap().id, 12);

        // Round 2: B1 now visible.
        let r2 = vec![
            vp(10, "work", "nvim"), // already resolved
            vp(11, "shell", "edit"),
            vp(12, "build", "cargo"), // already resolved
        ];
        resolve_restore_round(&mut store, &mut panes, &r2);
        assert_eq!(panes.len(), 3);
        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert_eq!(panes[1].as_ref().unwrap().id, 11);
        assert_eq!(panes[2].as_ref().unwrap().id, 12);
    }

    // ── Append-on-resolve for None-index bookmarks (post-freeze) ───────────

    #[test]
    fn none_index_bookmark_appends_and_rewrites_index() {
        // Post-freeze scenario: bookmark with index=None, should append.
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)), // already-resolved
                bm("shell", "edit", None),   // post-freeze append-on-resolve
            ],
            pane_id_to_bookmark_idx: [(10, 0)].into_iter().collect(),
            frozen: true,
        };
        let mut panes: Vec<Option<Pane>> = vec![Some(Pane {
            id: 10,
            tab_name: "work".to_owned(),
            pane_title: "nvim".to_owned(),
            tab_position: 0,
        })];

        let visible = vec![vp(10, "work", "nvim"), vp(11, "shell", "edit")];
        resolve_restore_round(&mut store, &mut panes, &visible);

        // Shell appended at end.
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[1].as_ref().unwrap().id, 11);
        // Bookmark's index rewritten to Some(1) (its new dense position).
        assert_eq!(store.bookmarks[1].index, Some(1));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&11), Some(&1));
    }

    // ── Already-resolved bookmarks skipped ──────────────────────────────────

    #[test]
    fn already_resolved_bookmarks_not_reprocessed() {
        // Bookmark already resolved (id 10 in map). Visible pane with same
        // (tab, title) and different id should NOT consume the bookmark again.
        let mut store = BookmarkStore {
            bookmarks: vec![bm("work", "nvim", Some(0))],
            pane_id_to_bookmark_idx: [(10, 0)].into_iter().collect(),
            frozen: false,
        };
        let mut panes: Vec<Option<Pane>> = vec![Some(Pane {
            id: 10,
            tab_name: "work".to_owned(),
            pane_title: "nvim".to_owned(),
            tab_position: 0,
        })];

        // Visible has a SECOND pane with same identity (id 99).
        let visible = vec![vp(10, "work", "nvim"), vp(99, "work", "nvim")];
        resolve_restore_round(&mut store, &mut panes, &visible);

        // panes unchanged — id 99 not registered, only one bookmark.
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert!(!store.pane_id_to_bookmark_idx.contains_key(&99));
    }

    // ── Duplicate-titled bookmarks distribute across duplicate-titled panes ─

    #[test]
    fn duplicate_bookmarks_distribute_to_duplicate_visible_panes() {
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("work", "nvim", Some(0)),
                bm("work", "nvim", Some(1)),
            ],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        let visible = vec![vp(10, "work", "nvim"), vp(11, "work", "nvim")];

        resolve_restore_round(&mut store, &mut panes, &visible);

        // First bookmark claims first matching pane; second bookmark claims
        // second pane.
        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert_eq!(panes[1].as_ref().unwrap().id, 11);
        assert_eq!(store.pane_id_to_bookmark_idx.get(&10), Some(&0));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&11), Some(&1));
    }

    // ── Non-bookmarked visible panes ignored ────────────────────────────────

    #[test]
    fn non_bookmarked_visible_panes_not_added() {
        let mut store = BookmarkStore {
            bookmarks: vec![bm("work", "nvim", Some(0))],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        let visible = vec![
            vp(10, "work", "nvim"),
            vp(99, "random", "no-bookmark"), // not a bookmark; ignored
        ];

        resolve_restore_round(&mut store, &mut panes, &visible);

        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert!(!store.pane_id_to_bookmark_idx.contains_key(&99));
    }

    // ── No bookmarks → no-op ────────────────────────────────────────────────

    #[test]
    fn empty_store_is_noop() {
        let mut store = BookmarkStore::default();
        let mut panes: Vec<Option<Pane>> = Vec::new();
        let visible = vec![vp(10, "work", "nvim")];

        resolve_restore_round(&mut store, &mut panes, &visible);

        assert!(panes.is_empty());
        assert!(store.pane_id_to_bookmark_idx.is_empty());
    }

    // ── Empty visible → no-op ───────────────────────────────────────────────

    #[test]
    fn empty_visible_with_pending_bookmarks_pre_sizes_only() {
        let mut store = BookmarkStore {
            bookmarks: vec![bm("a", "b", Some(2))],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        let visible = vec![];

        resolve_restore_round(&mut store, &mut panes, &visible);

        // Pre-sized to 3 (max_saved_idx + 1), all None.
        assert_eq!(panes.len(), 3);
        assert!(panes.iter().all(|p| p.is_none()));
    }

    // ── id-based resolution (title drift) ───────────────────────────────────

    fn bm_id(tab: &str, title: &str, index: Option<u16>, id: Option<u32>) -> PaneBookmark {
        PaneBookmark {
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            index,
            id,
        }
    }

    /// The headline fix: a bookmark whose saved title has drifted still
    /// resolves by its stable pane id, and the stored title is refreshed to
    /// the live one.
    #[test]
    fn id_match_resolves_despite_title_drift() {
        let mut store = BookmarkStore {
            bookmarks: vec![bm_id("work", "OLD TITLE", Some(0), Some(10))],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        // Same pane id 10, but the live title has changed.
        let visible = vec![vp(10, "work", "NEW TITLE")];

        resolve_restore_round(&mut store, &mut panes, &visible);

        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert_eq!(store.pane_id_to_bookmark_idx.get(&10), Some(&0));
        // Stored identity refreshed to the live title so disk stays current.
        assert_eq!(store.bookmarks[0].pane_title, "NEW TITLE");
        assert_eq!(store.bookmarks[0].id, Some(10));
    }

    /// id takes priority over title: a different visible pane that happens to
    /// share the bookmark's stale title is NOT claimed when the id matches a
    /// different pane.
    #[test]
    fn id_match_preferred_over_title_match() {
        let mut store = BookmarkStore {
            bookmarks: vec![bm_id("work", "shared", Some(0), Some(10))],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        // Pane 99 shares the title; pane 10 is the real one (title drifted).
        let visible = vec![vp(99, "work", "shared"), vp(10, "work", "drifted")];

        resolve_restore_round(&mut store, &mut panes, &visible);

        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert_eq!(store.pane_id_to_bookmark_idx.get(&10), Some(&0));
        assert!(!store.pane_id_to_bookmark_idx.contains_key(&99));
    }

    /// Cross-session fallback: the bookmark's saved id is no longer visible
    /// (ids were reassigned on a fresh session), so it falls back to
    /// (tab_name, pane_title) and adopts the new pane's id.
    #[test]
    fn title_fallback_when_saved_id_not_visible() {
        let mut store = BookmarkStore {
            bookmarks: vec![bm_id("work", "nvim", Some(0), Some(999))],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        // id 999 gone; a pane with the same (tab, title) exists under id 10.
        let visible = vec![vp(10, "work", "nvim")];

        resolve_restore_round(&mut store, &mut panes, &visible);

        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert_eq!(store.pane_id_to_bookmark_idx.get(&10), Some(&0));
        // Adopts the live id for subsequent id-based matches.
        assert_eq!(store.bookmarks[0].id, Some(10));
    }

    /// A bookmark with no id (loaded from a pre-id on-disk file) resolves by
    /// title and then adopts the live id.
    #[test]
    fn none_id_bookmark_resolves_by_title_then_adopts_id() {
        let mut store = BookmarkStore {
            bookmarks: vec![bm_id("work", "nvim", Some(0), None)],
            ..Default::default()
        };
        let mut panes: Vec<Option<Pane>> = Vec::new();
        let visible = vec![vp(10, "work", "nvim")];

        resolve_restore_round(&mut store, &mut panes, &visible);

        assert_eq!(panes[0].as_ref().unwrap().id, 10);
        assert_eq!(store.bookmarks[0].id, Some(10));
    }
}
