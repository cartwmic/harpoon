//! Bookmark data types: [`PaneBookmark`] (persisted) and [`BookmarkStore`]
//! (in-memory bookmark state, without I/O).
//!
//! `BookmarkStore` is the pure-data layer. The plugin shim (`harpoon-plugin`)
//! wraps it inside `Persistence`, which adds disk I/O and v1/v2 schema
//! migration on top. Having the data types here lets `harpoon-core` dispatch
//! handlers mutate the store without taking any `harpoon-plugin` dependency.
//!
//! See `openspec/changes/add-filter-and-jump-modes/design.md`:
//! - "Decision: Persistence schema v2 — envelope with single bookmarks Vec"
//! - "Decision: Persistence::pane_id_to_bookmark_idx is a first-class map"

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single persisted bookmark entry.
///
/// **v1 on-disk format** (legacy): `{"tab_name": "…", "pane_title": "…"}` —
/// no `index` field.  Deserializes with `index: None` via `#[serde(default)]`.
///
/// **v2 on-disk format**: `{"tab_name": "…", "pane_title": "…", "index": 2}` —
/// `index` is `Some(i)` for materialized bookmarks (currently in
/// `DispatchState::panes` at slot `i`) and `None` for bookmarks whose saved
/// position was frozen out or that were added before v2.
///
/// See `design.md` "Decision: Persistence schema v2 — envelope with single
/// bookmarks Vec".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneBookmark {
    pub tab_name: String,
    pub pane_title: String,
    /// Saved slot index in `DispatchState::panes`. `None` means "append on
    /// next restore" (either the bookmark was frozen out, or it was created
    /// before v2 and has never been explicitly placed).
    #[serde(default)]
    pub index: Option<u16>,
}

/// In-memory bookmark state for a single harpoon session.
///
/// The plugin shim owns a `Persistence` struct that wraps a `BookmarkStore`
/// and adds disk I/O on top. Keeping the pure-data half here allows
/// `harpoon-core` dispatch handlers (`a`/`d`/`K`/`J`/freeze) to operate on
/// the store without any `harpoon-plugin` or I/O dependency.
///
/// See `design.md`:
/// - "Decision: Persistence::pane_id_to_bookmark_idx is a first-class map"
/// - "Decision: state.panes is a sparse Vec<Option<Pane>>, made dense on
///   first user mutation"
#[derive(Debug, Clone, Default)]
pub struct BookmarkStore {
    /// Canonical ordered list of bookmarks. The plugin shim serializes this
    /// Vec directly as the `bookmarks` field of the v2 envelope — no rebuild
    /// from `state.panes` is ever needed.
    pub bookmarks: Vec<PaneBookmark>,

    /// Live-pane id → index into `self.bookmarks`. **NOT persisted**; rebuilt
    /// on load and on freeze. Maintained inline by add/delete/reorder/restore.
    ///
    /// Authoritative tracker for pane↔bookmark identity. See `design.md`
    /// "Decision: Persistence::pane_id_to_bookmark_idx is a first-class map".
    pub pane_id_to_bookmark_idx: HashMap<u32, usize>,

    /// `true` once `freeze_on_user_mutation` has run in this session.
    /// Mutation handlers may inspect this for guard logic; the flag is
    /// informational only (freeze itself is idempotent once panes are dense).
    pub frozen: bool,
}

impl BookmarkStore {
    /// Returns `true` iff some live-pane entry in `pane_id_to_bookmark_idx`
    /// points to `bookmark_idx`.
    ///
    /// Used by [`crate::freeze::freeze_on_user_mutation`] to determine which
    /// bookmarks have a currently-live pane and which are still pending
    /// restore (unresolved).
    pub fn is_resolved(&self, bookmark_idx: usize) -> bool {
        self.pane_id_to_bookmark_idx
            .values()
            .any(|&v| v == bookmark_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PaneBookmark serde ────────────────────────────────────────────────────

    #[test]
    fn pane_bookmark_v2_round_trip() {
        let bm = PaneBookmark {
            tab_name: "a".to_owned(),
            pane_title: "b".to_owned(),
            index: Some(3),
        };
        let json = serde_json::to_string(&bm).expect("serialize");
        let got: PaneBookmark = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(got, bm);
    }

    #[test]
    fn pane_bookmark_v1_backward_compat() {
        // v1 files have no `index` field; must deserialize with index: None.
        let json = r#"{"tab_name":"a","pane_title":"b"}"#;
        let got: PaneBookmark = serde_json::from_str(json).expect("deserialize v1");
        assert_eq!(
            got,
            PaneBookmark {
                tab_name: "a".to_owned(),
                pane_title: "b".to_owned(),
                index: None,
            }
        );
    }

    // ── BookmarkStore::default ────────────────────────────────────────────────

    #[test]
    fn bookmark_store_default_is_empty_and_not_frozen() {
        let store = BookmarkStore::default();
        assert!(store.bookmarks.is_empty());
        assert!(store.pane_id_to_bookmark_idx.is_empty());
        assert!(!store.frozen);
    }

    // ── BookmarkStore::is_resolved ────────────────────────────────────────────

    #[test]
    fn is_resolved_true_when_map_points_at_idx() {
        let mut store = BookmarkStore::default();
        store.bookmarks.push(PaneBookmark {
            tab_name: "work".to_owned(),
            pane_title: "nvim".to_owned(),
            index: Some(0),
        });
        store.pane_id_to_bookmark_idx.insert(42, 0);
        assert!(store.is_resolved(0));
    }

    #[test]
    fn is_resolved_false_when_no_map_entry_points_at_idx() {
        let mut store = BookmarkStore::default();
        store.bookmarks.push(PaneBookmark {
            tab_name: "work".to_owned(),
            pane_title: "nvim".to_owned(),
            index: Some(0),
        });
        // No entry in pane_id_to_bookmark_idx — bookmark is unresolved.
        assert!(!store.is_resolved(0));
    }

    #[test]
    fn is_resolved_false_for_unmapped_index_even_with_other_entries() {
        let mut store = BookmarkStore::default();
        store.bookmarks.push(PaneBookmark {
            tab_name: "a".to_owned(),
            pane_title: "b".to_owned(),
            index: Some(0),
        });
        store.bookmarks.push(PaneBookmark {
            tab_name: "c".to_owned(),
            pane_title: "d".to_owned(),
            index: Some(1),
        });
        // Only idx 0 is resolved; idx 1 has no map value pointing at it.
        store.pane_id_to_bookmark_idx.insert(1, 0);
        assert!(store.is_resolved(0));
        assert!(!store.is_resolved(1));
    }
}
