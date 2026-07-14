//! Render-layer string builders.
//!
//! Pure layout logic for the harpoon UI: header (mode-aware, with
//! narrow-width truncation), pane rows (with slot prefixes, placeholder
//! handling, match highlighting), and the hint line (mode-aware budgets at
//! 80/50/30 column widths).
//!
//! Returns structured descriptors (`RenderRow`, `RenderHeader`) that the
//! plugin shim translates into `zellij_tile::Text` calls. Keeping layout
//! decisions natively-testable is the explicit testability gate from
//! design.md "Decision: Render builders extracted to harpoon-core".

use std::ops::Range;

use crate::bookmark::BookmarkStore;
use crate::dispatch::DispatchState;
use crate::matcher::{Matcher, MatcherImpl};
use crate::mode::Mode;
use crate::slot::slot_char_from_index;

// ── Descriptor types ──────────────────────────────────────────────────────────

/// One rendered row (either a Live pane or a placeholder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderRow {
    /// The full text to render (slot prefix + display string, or placeholder
    /// `"<slot>  ?  (resolving)"`).
    pub text: String,
    /// Character indices (NOT byte offsets) to apply match highlighting.
    /// Empty for placeholder rows and command/jump-mode rows.
    pub highlight_indices: Vec<usize>,
    pub highlight_kind: HighlightKind,
    pub is_selected: bool,
    /// `true` if this row represents an unresolved bookmark (None slot).
    pub is_placeholder: bool,
}

/// One header line (top-of-pane). The header can be 1 line (standard
/// `==== N panes ====` or `/query (m/n)`) or 2 lines for narrow-width
/// truncation where the badge gets its own line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderLine {
    pub text: String,
    /// Character range of the `[N]/[F]/[J]` badge for color emphasis. `None`
    /// if the badge is not on this line.
    pub badge_range: Option<Range<usize>>,
    /// Character range of the query string for color emphasis (filter mode
    /// only). `None` otherwise.
    pub query_range: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderHeader {
    pub lines: Vec<HeaderLine>,
    /// Number of rows the header consumes. The shim renders panes starting
    /// at `HEADER_BASE_Y + height`.
    pub height: u16,
}

/// How to apply highlight indices.
///
/// - `None`: no highlight (placeholder rows, command/jump-mode rows).
/// - `FuzzyChars`: `highlight_indices` is a list of (potentially non-
///   contiguous) char positions; the shim uses `Text::color_indices` because
///   `color_range` only accepts a single contiguous range.
/// - `SubstringRange { start, end }`: `start..end` is a single contiguous
///   range; the shim uses `Text::color_range` for efficiency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    None,
    FuzzyChars,
    SubstringRange { start: usize, end: usize },
}

/// One row entry in the build_rows input. The shim builds this from
/// `state.panes` + `BookmarkStore::bookmarks` (placeholder lookup).
#[derive(Debug, Clone)]
pub enum RowEntry<'a> {
    Live(&'a crate::pane::Pane),
    Placeholder {
        saved_tab_name: String,
        saved_pane_title: String,
    },
}

// ── build_row_entries ────────────────────────────────────────────────────────

/// Produce the per-slot row entries from the sparse `state.panes` Vec and
/// `BookmarkStore`. Live panes win over placeholders at the same index;
/// `None` slots become placeholders sourced from the matching bookmark
/// (the unresolved `Some(_)` index entry).
///
/// Output length is `max(panes.len(), max_unresolved_saved_index + 1)`, so
/// that placeholder slots beyond the current resolved set are still rendered.
pub fn build_row_entries<'a>(state: &'a DispatchState, store: &BookmarkStore) -> Vec<RowEntry<'a>> {
    // Collect placeholder data keyed on slot index.
    let mut placeholders: std::collections::HashMap<usize, (String, String)> =
        std::collections::HashMap::new();
    let mut max_saved_idx = 0usize;
    let mut append_placeholders: Vec<(String, String)> = Vec::new();
    for (bk_idx, b) in store.bookmarks.iter().enumerate() {
        match b.index {
            Some(idx) => {
                let i = idx as usize;
                if i + 1 > max_saved_idx {
                    max_saved_idx = i + 1;
                }
                // Only render as placeholder if the bookmark is NOT resolved
                // (no live pane mapped to it).
                if !store.is_resolved(bk_idx) {
                    placeholders.insert(i, (b.tab_name.clone(), b.pane_title.clone()));
                }
            }
            None if !store.is_resolved(bk_idx) => {
                // Persisted post-freeze bookmarks are valid with no slot.
                // Project them after indexed rows so first-render fullness is
                // satisfiable even before their pane becomes visible.
                append_placeholders.push((b.tab_name.clone(), b.pane_title.clone()));
            }
            None => {}
        }
    }

    let total = state.panes.len().max(max_saved_idx);
    let mut entries: Vec<RowEntry<'a>> = Vec::with_capacity(total);

    for i in 0..total {
        // Live wins at this index if present.
        if let Some(Some(p)) = state.panes.get(i) {
            entries.push(RowEntry::Live(p));
            continue;
        }
        if let Some((tab, title)) = placeholders.remove(&i) {
            entries.push(RowEntry::Placeholder {
                saved_tab_name: tab,
                saved_pane_title: title,
            });
            continue;
        }
        // Slot index past panes.len() and no placeholder bookmark — skip.
        // (This shouldn't happen given our `total` calc above, but bail out
        // safely.)
        break;
    }
    entries.extend(
        append_placeholders
            .into_iter()
            .map(|(saved_tab_name, saved_pane_title)| RowEntry::Placeholder {
                saved_tab_name,
                saved_pane_title,
            }),
    );
    entries
}

// ── build_header ─────────────────────────────────────────────────────────────

/// Build the header descriptor for the given mode/state.
///
/// `cols` is the available column budget; `max_height` caps how many lines
/// the header may consume (`1` for very small panes, `2` otherwise).
///
/// Layout rules:
/// - Command/Jump (or Filter with empty query): single line
///   `"==== <N> panes ====   [<badge>]"`. The count takes priority; the
///   badge is appended right-aligned. On narrow widths the count drops to
///   `"<N> panes"` then `"<N>"`, then the badge floats to its own line if
///   `max_height >= 2`.
/// - Filter with non-empty query: single line `"/<query>  (<m>/<n>)  [F]"`.
///   On narrow: drop count, then truncate query with leading ellipsis.
///
/// For the simple version (single-line layout always) we handle the common
/// case; narrow-width 2-line behavior is implemented but conservative.
pub fn build_header(
    state: &DispatchState,
    visible_count: usize,
    filter_match_count: usize,
    cols: usize,
    max_height: u16,
) -> RenderHeader {
    let badge = format!("[{}]", state.mode.badge_letter());

    let body = match state.mode {
        Mode::Filter if !state.query.is_empty() => {
            // "/<query>  (<m>/<n>)"
            let count = format!("({}/{})", filter_match_count, visible_count);
            (format!("/{}", state.query), Some(count))
        }
        _ => {
            let body = format!("==== {} panes ====", visible_count);
            (body, None)
        }
    };

    let mut single_line_text = body.0.clone();
    if let Some(count) = &body.1 {
        single_line_text.push_str("  ");
        single_line_text.push_str(count);
    }

    // Append badge with at-least-one-space separator. If we can fit it on
    // the same line, do so; else split to a second line if max_height >= 2.
    let single_line_total = single_line_text.chars().count() + 1 + badge.chars().count();
    if single_line_total <= cols || max_height < 2 {
        // Fit (or forced single-line): inline the badge on the right.
        let mut text = single_line_text.clone();
        text.push(' ');
        let badge_start = text.chars().count();
        text.push_str(&badge);
        let badge_end = text.chars().count();
        let query_range = match (&state.mode, body.1.is_some()) {
            (Mode::Filter, true) if !state.query.is_empty() => {
                // "/" at index 0; query chars at 1..(1 + query.len())
                Some(1..(1 + state.query.chars().count()))
            }
            _ => None,
        };
        let line = HeaderLine {
            text,
            badge_range: Some(badge_start..badge_end),
            query_range,
        };
        return RenderHeader {
            lines: vec![line],
            height: 1,
        };
    }

    // Two-line layout: body + count on line 0, badge on line 1.
    let body_text = single_line_text;
    let query_range_l0 = match (&state.mode, body.1.is_some()) {
        (Mode::Filter, true) if !state.query.is_empty() => {
            Some(1..(1 + state.query.chars().count()))
        }
        _ => None,
    };
    let line0 = HeaderLine {
        text: body_text,
        badge_range: None,
        query_range: query_range_l0,
    };
    let badge_chars = badge.chars().count();
    let line1 = HeaderLine {
        text: badge.clone(),
        badge_range: Some(0..badge_chars),
        query_range: None,
    };
    RenderHeader {
        lines: vec![line0, line1],
        height: 2,
    }
}

// ── build_rows ───────────────────────────────────────────────────────────────

/// Build per-row descriptors. In Filter mode with non-empty query, only Live
/// matches in score order; otherwise all entries (Live + placeholders) in
/// declared order.
///
/// `show_slots` controls whether the `<slot>  ` prefix is rendered (always
/// suppressed in filter mode).
pub fn build_rows(
    state: &DispatchState,
    entries: &[RowEntry<'_>],
    matcher: &mut MatcherImpl,
    filtered_indices: &[usize],
    show_slots: bool,
    max_rows: u16,
) -> Vec<RenderRow> {
    let in_active_filter = matches!(state.mode, Mode::Filter) && !state.query.is_empty();
    let mut rows: Vec<RenderRow> = Vec::new();

    if in_active_filter {
        // Iterate filtered_indices (already score-ordered with tie-breaker).
        // Placeholders are excluded from filter view per filter.rs::filtered_indices.
        for (rendered_idx, &panes_idx) in filtered_indices.iter().enumerate() {
            if rendered_idx as u16 >= max_rows {
                break;
            }
            let entry = match entries.get(panes_idx) {
                Some(RowEntry::Live(p)) => p,
                _ => continue, // shouldn't happen — filter excludes placeholders
            };
            let display = format!("{}", entry); // Pane Display = "tab | title"
            let (highlight_kind, highlight_indices) =
                compute_highlight(matcher, &display, &state.query);
            // No slot prefix in filter mode — set selected based on `state.selected`
            // which indexes filtered_indices when filter active.
            let is_selected = state.selected == rendered_idx;
            rows.push(RenderRow {
                text: display,
                highlight_indices,
                highlight_kind,
                is_selected,
                is_placeholder: false,
            });
        }
    } else {
        // Command / Jump / Filter+empty-query: walk entries in declared order.
        for (i, entry) in entries.iter().enumerate() {
            if i as u16 >= max_rows {
                break;
            }
            let prefix = slot_prefix(i, show_slots);
            let (text, is_placeholder) = match entry {
                RowEntry::Live(p) => (format!("{}{}", prefix, p), false),
                RowEntry::Placeholder {
                    saved_tab_name,
                    saved_pane_title,
                } => (
                    format!(
                        "{}{} | {}  (resolving)",
                        prefix, saved_tab_name, saved_pane_title
                    ),
                    true,
                ),
            };
            let is_selected = state.selected == i;
            rows.push(RenderRow {
                text,
                highlight_indices: Vec::new(),
                highlight_kind: HighlightKind::None,
                is_selected,
                is_placeholder,
            });
        }
    }

    rows
}

/// Compute the slot prefix for a row. Empty string when `show_slots = false`
/// or filter mode (caller decides to skip).
///
/// Format: `"<slot>  "` (3 chars total: `<char><space><space>`) for slots
/// 0..35; `"   "` (3 spaces) padding for index 35+.
fn slot_prefix(i: usize, show_slots: bool) -> String {
    if !show_slots {
        return String::new();
    }
    match slot_char_from_index(i) {
        Some(c) => format!("{}  ", c),
        None => "   ".to_owned(),
    }
}

/// Run the matcher against a haystack and produce highlight metadata.
fn compute_highlight(
    matcher: &mut MatcherImpl,
    haystack: &str,
    needle: &str,
) -> (HighlightKind, Vec<usize>) {
    let Some((_score, indices)) = matcher.match_indices(haystack, needle) else {
        return (HighlightKind::None, Vec::new());
    };
    if indices.is_empty() {
        return (HighlightKind::None, Vec::new());
    }
    // Detect contiguous range so the shim can use `Text::color_range` instead
    // of `Text::color_indices` for substring matches.
    let start = *indices.first().unwrap();
    let end = *indices.last().unwrap() + 1;
    let is_contiguous = end - start == indices.len();
    if is_contiguous {
        (HighlightKind::SubstringRange { start, end }, indices)
    } else {
        (HighlightKind::FuzzyChars, indices)
    }
}

// ── build_hint_line ──────────────────────────────────────────────────────────

/// Build the bottom-of-pane hint line (mode-aware). Caller passes available
/// column budget; the function progressively shortens the hint to stay within.
///
/// At ≥ 80 cols: full key labels.
/// At ≥ 50 cols: drop labels, keep keys.
/// At < 50 cols: minimal keys only.
pub fn build_hint_line(mode: Mode, cols: usize) -> String {
    match mode {
        Mode::Command => {
            if cols >= 80 {
                // Keep <= 80 chars total (currently 76).
                "<a/A>add <d>del <K/J>reorder <1-9>jump </>filter <#>jump <c/Esc>close".to_owned()
            } else if cols >= 50 {
                // 50 cols.
                "<a/A>add <d>del <K/J>swap <1-9>jump </> <#> <Esc>".to_owned()
            } else if cols >= 30 {
                "a/A/d K/J 1-9 / # Esc".to_owned()
            } else {
                "a A d K J".to_owned()
            }
        }
        Mode::Filter => {
            if cols >= 80 {
                "<Esc> clear/exit  <Enter> focus  <\u{2191}/\u{2193}> nav".to_owned()
            } else if cols >= 50 {
                "<Esc>clear <Enter>focus <\u{2191}/\u{2193}>nav".to_owned()
            } else if cols >= 30 {
                "<Esc/Enter/\u{2191}/\u{2193}>".to_owned()
            } else {
                "Esc/\u{21B5}".to_owned()
            }
        }
        Mode::Jump => {
            if cols >= 80 {
                "<1-9/a-z> jump  <Esc> back".to_owned()
            } else if cols >= 50 {
                "<1-9/a-z>jump <Esc>back".to_owned()
            } else if cols >= 30 {
                "<1-9/a-z/Esc>".to_owned()
            } else {
                "1-9 a-z Esc".to_owned()
            }
        }
    }
}

// ── Layout precedence helper ──────────────────────────────────────────────────

/// Compute available row count for pane rows given total `rows`, header
/// height, and whether the hint line is visible.
///
/// Layout precedence (from `design.md` "Decision: Tiny-pane layout
/// precedence"):
/// 1. Drop hint line first if `rows.saturating_sub(header.height) < 2`.
/// 2. Collapse 2-line header to single line if `rows < 4`.
/// 3. Drop header if `rows < 2`.
/// Pane rows always survive.
pub struct LayoutBudget {
    pub max_header_height: u16,
    pub hint_visible: bool,
    pub max_pane_rows: u16,
}

pub fn compute_layout_budget(rows: u16) -> LayoutBudget {
    let max_header_height = if rows >= 4 { 2 } else { 1 };
    // Reserve 1 row for hint when we have header + at least 1 row of panes.
    let hint_visible = rows >= 4;
    let used = if hint_visible { 1 } else { 0 } + max_header_height;
    let max_pane_rows = rows.saturating_sub(used);
    LayoutBudget {
        max_header_height,
        hint_visible,
        max_pane_rows,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmark::PaneBookmark;
    use crate::matcher::SubstringMatcher;
    use crate::pane::Pane;

    fn p(id: u32, tab: &str, title: &str) -> Pane {
        Pane {
            id,
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            tab_position: 0,
        }
    }

    fn substring_matcher() -> MatcherImpl {
        MatcherImpl::Substring(SubstringMatcher::new())
    }

    // ── slot_prefix ──────────────────────────────────────────────────────

    #[test]
    fn slot_prefix_show_slots_off_returns_empty() {
        assert_eq!(slot_prefix(0, false), "");
        assert_eq!(slot_prefix(34, false), "");
    }

    #[test]
    fn slot_prefix_digit_slots() {
        assert_eq!(slot_prefix(0, true), "1  ");
        assert_eq!(slot_prefix(8, true), "9  ");
    }

    #[test]
    fn slot_prefix_letter_slots() {
        assert_eq!(slot_prefix(9, true), "a  ");
        assert_eq!(slot_prefix(34, true), "z  ");
    }

    #[test]
    fn slot_prefix_beyond_35_pads_with_spaces() {
        assert_eq!(slot_prefix(35, true), "   ");
        assert_eq!(slot_prefix(99, true), "   ");
    }

    // ── build_row_entries ────────────────────────────────────────────────

    #[test]
    fn build_row_entries_all_live() {
        let mut s = DispatchState::default();
        let p0 = p(1, "a", "x");
        let p1 = p(2, "b", "y");
        s.panes = vec![Some(p0.clone()), Some(p1.clone())];
        let store = BookmarkStore::default();
        let entries = build_row_entries(&s, &store);
        assert_eq!(entries.len(), 2);
        match &entries[0] {
            RowEntry::Live(_) => {}
            _ => panic!(),
        }
        match &entries[1] {
            RowEntry::Live(_) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn build_row_entries_with_placeholder() {
        let mut s = DispatchState::default();
        let p0 = p(1, "work", "nvim");
        let p2 = p(3, "build", "cargo");
        s.panes = vec![Some(p0.clone()), None, Some(p2.clone())];
        let mut store = BookmarkStore::default();
        store.bookmarks = vec![
            PaneBookmark {
                tab_name: "work".to_owned(),
                pane_title: "nvim".to_owned(),
                index: Some(0),
                id: None,
            },
            PaneBookmark {
                tab_name: "shell".to_owned(),
                pane_title: "edit".to_owned(),
                index: Some(1),
                id: None,
            },
            PaneBookmark {
                tab_name: "build".to_owned(),
                pane_title: "cargo".to_owned(),
                index: Some(2),
                id: None,
            },
        ];
        store.pane_id_to_bookmark_idx.insert(1, 0);
        store.pane_id_to_bookmark_idx.insert(3, 2);
        // bookmark 1 is unresolved (no map entry pointing at idx 1)

        let entries = build_row_entries(&s, &store);
        assert_eq!(entries.len(), 3);
        assert!(matches!(entries[0], RowEntry::Live(_)));
        match &entries[1] {
            RowEntry::Placeholder {
                saved_tab_name,
                saved_pane_title,
            } => {
                assert_eq!(saved_tab_name, "shell");
                assert_eq!(saved_pane_title, "edit");
            }
            _ => panic!("expected placeholder at index 1"),
        }
        assert!(matches!(entries[2], RowEntry::Live(_)));
    }

    // ── build_header ─────────────────────────────────────────────────────

    #[test]
    fn header_command_mode_single_line() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        let h = build_header(&s, 5, 0, 80, 2);
        assert_eq!(h.height, 1);
        assert_eq!(h.lines.len(), 1);
        assert!(h.lines[0].text.contains("==== 5 panes ===="));
        assert!(h.lines[0].text.contains("[N]"));
        assert!(h.lines[0].badge_range.is_some());
    }

    #[test]
    fn header_filter_mode_with_query() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "ed".to_owned();
        let h = build_header(&s, 5, 2, 80, 2);
        assert_eq!(h.height, 1);
        assert!(h.lines[0].text.contains("/ed"));
        assert!(h.lines[0].text.contains("(2/5)"));
        assert!(h.lines[0].text.contains("[F]"));
        // Query range should cover chars 1..3 (after the leading '/')
        assert_eq!(h.lines[0].query_range, Some(1..3));
    }

    #[test]
    fn header_jump_mode() {
        let mut s = DispatchState::default();
        s.mode = Mode::Jump;
        let h = build_header(&s, 3, 0, 80, 2);
        assert!(h.lines[0].text.contains("[J]"));
    }

    #[test]
    fn header_narrow_width_two_line_layout() {
        // Force narrow width that can't fit body + badge on one line.
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "verylong".to_owned();
        let h = build_header(&s, 100, 5, 15, 2); // 15 cols, max_height=2
        assert_eq!(h.height, 2);
        assert_eq!(h.lines.len(), 2);
        assert!(h.lines[1].text.contains("[F]"));
    }

    #[test]
    fn header_max_height_1_forces_single_line_even_when_too_wide() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        let h = build_header(&s, 999999, 0, 5, 1);
        assert_eq!(h.height, 1, "max_height=1 forces single line");
    }

    // ── build_rows ───────────────────────────────────────────────────────

    #[test]
    fn rows_command_mode_with_slot_prefix() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x")), Some(p(2, "b", "y"))];
        s.selected = 0;
        let entries = build_row_entries(&s, &BookmarkStore::default());
        let mut m = substring_matcher();
        let rows = build_rows(&s, &entries, &mut m, &[], true, 100);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].text.starts_with("1  "));
        assert!(rows[1].text.starts_with("2  "));
        assert!(rows[0].is_selected);
        assert!(!rows[1].is_selected);
    }

    #[test]
    fn rows_command_mode_without_slot_prefix() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x"))];
        let entries = build_row_entries(&s, &BookmarkStore::default());
        let mut m = substring_matcher();
        let rows = build_rows(&s, &entries, &mut m, &[], false, 100);
        assert!(!rows[0].text.starts_with("1  "));
        assert_eq!(rows[0].text, "a | x");
    }

    #[test]
    fn rows_placeholder_renders_resolving_text() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x")), None];
        let mut store = BookmarkStore::default();
        store.bookmarks = vec![
            PaneBookmark {
                tab_name: "a".to_owned(),
                pane_title: "x".to_owned(),
                index: Some(0),
                id: None,
            },
            PaneBookmark {
                tab_name: "b".to_owned(),
                pane_title: "y".to_owned(),
                index: Some(1),
                id: None,
            },
        ];
        store.pane_id_to_bookmark_idx.insert(1, 0);
        let entries = build_row_entries(&s, &store);
        let mut m = substring_matcher();
        let rows = build_rows(&s, &entries, &mut m, &[], true, 100);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].is_placeholder);
        assert!(rows[1].is_placeholder);
        assert!(rows[1].text.contains("b | y  (resolving)"));
        assert!(rows[1].text.starts_with("2  "));
    }

    #[test]
    fn rows_filter_mode_suppresses_slot_prefix() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "x".to_owned();
        s.panes = vec![Some(p(1, "a", "x")), Some(p(2, "b", "y"))];
        s.selected = 0;
        let entries = build_row_entries(&s, &BookmarkStore::default());
        let mut m = substring_matcher();
        let filtered = vec![0]; // Only panes[0] matches "x"
        let rows = build_rows(&s, &entries, &mut m, &filtered, true, 100);
        assert_eq!(rows.len(), 1);
        assert!(
            !rows[0].text.starts_with("1  "),
            "no slot prefix in filter mode"
        );
        assert!(rows[0].is_selected);
    }

    #[test]
    fn rows_filter_mode_substring_highlight_is_contiguous_range() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "edit".to_owned();
        s.panes = vec![Some(p(1, "shell", "edit log"))];
        let entries = build_row_entries(&s, &BookmarkStore::default());
        let mut m = substring_matcher();
        let filtered = vec![0];
        let rows = build_rows(&s, &entries, &mut m, &filtered, true, 100);
        assert_eq!(rows.len(), 1);
        match rows[0].highlight_kind {
            HighlightKind::SubstringRange { start, end } => {
                assert_eq!(start, 8); // "shell | " is 8 chars
                assert_eq!(end, 12); // "edit" is 4 chars → 8..12
            }
            other => panic!("expected SubstringRange, got {:?}", other),
        }
    }

    #[test]
    fn rows_max_rows_caps_output() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = (0..20).map(|i| Some(p(i, "a", "x"))).collect();
        let entries = build_row_entries(&s, &BookmarkStore::default());
        let mut m = substring_matcher();
        let rows = build_rows(&s, &entries, &mut m, &[], true, 5);
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn rows_multibyte_haystack_substring_highlight_at_char_position() {
        // "📦 build | tail log" + needle "log".
        // chars: 📦(0) (1) b(2) u(3) i(4) l(5) d(6) (7) |(8) (9) t(10) a(11) i(12) l(13) (14) l(15) o(16) g(17)
        // Expect highlight at chars [15, 16, 17] → range 15..18.
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "log".to_owned();
        // Use a Pane whose Display = "📦 build | tail log" — set tab_name = "📦 build" and pane_title = "tail log"
        // Display impl is "{tab_name} | {pane_title}".
        s.panes = vec![Some(p(1, "📦 build", "tail log"))];
        let entries = build_row_entries(&s, &BookmarkStore::default());
        let mut m = substring_matcher();
        let filtered = vec![0];
        let rows = build_rows(&s, &entries, &mut m, &filtered, true, 100);
        match rows[0].highlight_kind {
            HighlightKind::SubstringRange { start, end } => {
                assert_eq!(start, 15);
                assert_eq!(end, 18);
            }
            other => panic!("expected SubstringRange, got {:?}", other),
        }
    }

    // ── build_hint_line ──────────────────────────────────────────────────

    #[test]
    fn hint_command_at_80_includes_keys() {
        let h = build_hint_line(Mode::Command, 80);
        assert!(h.contains("a"));
        assert!(h.contains("d"));
        assert!(h.contains("Esc"));
        assert!(h.contains("K/J"));
        assert!(h.contains("1-9"));
    }

    #[test]
    fn hint_at_each_budget_fits_within_cols() {
        for &cols in &[80usize, 50, 30, 20] {
            for mode in [Mode::Command, Mode::Filter, Mode::Jump] {
                let h = build_hint_line(mode, cols);
                assert!(
                    h.chars().count() <= cols,
                    "hint for {:?} at cols={} exceeds: {}",
                    mode,
                    cols,
                    h
                );
            }
        }
    }

    // ── compute_layout_budget ────────────────────────────────────────────

    #[test]
    fn layout_budget_at_24_rows() {
        let b = compute_layout_budget(24);
        assert_eq!(b.max_header_height, 2);
        assert!(b.hint_visible);
        assert_eq!(b.max_pane_rows, 21); // 24 - 1 hint - 2 header
    }

    #[test]
    fn layout_budget_at_4_rows() {
        let b = compute_layout_budget(4);
        assert_eq!(b.max_header_height, 2);
        assert!(b.hint_visible);
        assert_eq!(b.max_pane_rows, 1); // 4 - 1 - 2 = 1
    }

    #[test]
    fn layout_budget_at_3_rows() {
        // < 4: hint dropped, header collapses to 1 line.
        let b = compute_layout_budget(3);
        assert_eq!(b.max_header_height, 1);
        assert!(!b.hint_visible);
        assert_eq!(b.max_pane_rows, 2);
    }

    #[test]
    fn layout_budget_at_2_rows() {
        let b = compute_layout_budget(2);
        assert_eq!(b.max_header_height, 1);
        assert!(!b.hint_visible);
        assert_eq!(b.max_pane_rows, 1);
    }

    #[test]
    fn layout_budget_at_1_row() {
        let b = compute_layout_budget(1);
        assert_eq!(b.max_header_height, 1);
        assert!(!b.hint_visible);
        assert_eq!(b.max_pane_rows, 0);
    }
}

// Display impl for RowEntry to make it usable in format!() above (as &Pane).
impl<'a> std::fmt::Display for RowEntry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RowEntry::Live(p) => write!(f, "{}", p),
            RowEntry::Placeholder {
                saved_tab_name,
                saved_pane_title,
            } => write!(f, "{} | {}", saved_tab_name, saved_pane_title),
        }
    }
}
