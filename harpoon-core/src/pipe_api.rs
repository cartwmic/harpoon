//! Pure helpers backing the plugin's external CLI-pipe interface
//! (`pane-pipe-api` capability): pane-id string parsing and pane→slot reverse
//! lookup. Kept in `harpoon-core` so they are unit-testable natively, free of
//! any `zellij-tile` dependency. The plugin shim (`harpoon-plugin`) calls these
//! from its `pipe()` handler and wires the results to `cli_pipe_output` /
//! `jump_focus_fullscreen`.

use crate::bookmark::BookmarkStore;

/// Parse a zellij pane-id string into a terminal pane id.
///
/// Accepts both forms zellij uses for terminal panes:
/// - `terminal_N` — the form exported to every pane as `$ZELLIJ_PANE_ID`.
/// - bare `N` — the integer form (equivalent to `terminal_N`).
///
/// Returns `None` for anything else: an empty string, a non-numeric tail, a
/// non-terminal pane kind (e.g. `plugin_3`), or a value that does not fit a
/// `u32`. Matching the string form here reconciles `$ZELLIJ_PANE_ID` with
/// harpoon's stored `PaneInfo.id` (`u32`).
///
/// AC: `pane-pipe-api.pane-id-string-parsing`.
pub fn parse_pane_id(raw: &str) -> Option<u32> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let digits = s.strip_prefix("terminal_").unwrap_or(s);
    // Reject any other underscore-tagged kind (e.g. `plugin_3`) and empty tails.
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

/// Reverse lookup: the 1-based harpoon slot currently holding terminal pane
/// `id`, or `None` when the pane has no bookmark or its bookmark is not
/// materialized into a slot (`PaneBookmark.index` is `None`).
///
/// The returned value is `PaneBookmark.index + 1` — the 1-based form of the
/// slot index in `DispatchState::panes` (the position a user hotkeys in jump
/// mode). Pure read: never mutates `store`.
///
/// AC: `pane-pipe-api.slot-for-pane-reverse-lookup`.
pub fn slot_for_pane(store: &BookmarkStore, id: u32) -> Option<u16> {
    let &bi = store.pane_id_to_bookmark_idx.get(&id)?;
    let bookmark = store.bookmarks.get(bi)?;
    bookmark.index.map(|i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmark::{BookmarkStore, PaneBookmark};

    // ── parse_pane_id ─ AC pane-pipe-api.pane-id-string-parsing ──────────────

    #[test]
    fn parses_terminal_prefixed_form() {
        assert_eq!(parse_pane_id("terminal_7"), Some(7));
    }

    #[test]
    fn parses_bare_integer_form() {
        assert_eq!(parse_pane_id("7"), Some(7));
    }

    #[test]
    fn parses_zero() {
        assert_eq!(parse_pane_id("terminal_0"), Some(0));
        assert_eq!(parse_pane_id("0"), Some(0));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        // zellij pipe payloads can carry a trailing newline.
        assert_eq!(parse_pane_id(" terminal_12\n"), Some(12));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(parse_pane_id(""), None);
        assert_eq!(parse_pane_id("   "), None);
    }

    #[test]
    fn rejects_plugin_kind() {
        assert_eq!(parse_pane_id("plugin_3"), None);
    }

    #[test]
    fn rejects_non_numeric() {
        assert_eq!(parse_pane_id("abc"), None);
        assert_eq!(parse_pane_id("terminal_"), None);
        assert_eq!(parse_pane_id("terminal_x"), None);
        assert_eq!(parse_pane_id("7a"), None);
    }

    #[test]
    fn rejects_overflow() {
        // u32::MAX + 1 does not fit.
        assert_eq!(parse_pane_id("4294967296"), None);
    }

    // ── slot_for_pane ─ AC pane-pipe-api.slot-for-pane-reverse-lookup ────────

    fn bm(tab: &str, title: &str, index: Option<u16>, id: Option<u32>) -> PaneBookmark {
        PaneBookmark {
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            index,
            id,
        }
    }

    /// Build a store with one bookmark at slot index 2 bound to pane id 7.
    fn store_with_pane7_at_slot2() -> BookmarkStore {
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("a", "p0", Some(0), Some(5)),
                bm("b", "p1", Some(1), Some(6)),
                bm("c", "p7", Some(2), Some(7)),
            ],
            ..Default::default()
        };
        store.pane_id_to_bookmark_idx.insert(5, 0);
        store.pane_id_to_bookmark_idx.insert(6, 1);
        store.pane_id_to_bookmark_idx.insert(7, 2);
        store
    }

    #[test]
    fn harpooned_pane_returns_one_based_slot() {
        let store = store_with_pane7_at_slot2();
        // slot index 2 → 1-based slot 3
        assert_eq!(slot_for_pane(&store, 7), Some(3));
    }

    #[test]
    fn absent_pane_returns_none() {
        let store = store_with_pane7_at_slot2();
        assert_eq!(slot_for_pane(&store, 999), None);
    }

    #[test]
    fn unmaterialized_bookmark_returns_none() {
        // Bookmark exists and is mapped to a live pane, but has no slot index.
        let mut store = BookmarkStore {
            bookmarks: vec![bm("a", "p", None, Some(42))],
            ..Default::default()
        };
        store.pane_id_to_bookmark_idx.insert(42, 0);
        assert_eq!(slot_for_pane(&store, 42), None);
    }

    #[test]
    fn lookup_does_not_mutate_store() {
        let store = store_with_pane7_at_slot2();
        let before = store.clone();
        let _ = slot_for_pane(&store, 7);
        let _ = slot_for_pane(&store, 999);
        assert_eq!(store.bookmarks, before.bookmarks);
        assert_eq!(store.pane_id_to_bookmark_idx, before.pane_id_to_bookmark_idx);
        assert_eq!(store.frozen, before.frozen);
    }
}
