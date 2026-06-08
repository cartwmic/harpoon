//! Filter-mode key dispatch.
//!
//! In filter mode, printable characters append to `state.query` (the buffer
//! the matcher consumes). Backspace pops; arrow keys nav within the filtered
//! view; Enter focuses the selected match; Esc with non-empty query clears
//! it (re-anchoring to focused pane), Esc with empty query returns to
//! command mode.
//!
//! See `specs/filter-mode/spec.md` for the full contract. Modifier gating is
//! strict: only `ModifierSet::PLAIN` printables append (per FFI normalization,
//! Shift on letters is collapsed before reaching the handler, so `is_plain()`
//! is sufficient). `Ctrl+W`, `Ctrl+C`, `Alt+x` etc. return `vec![Effect::Noop]`
//! to avoid silently swallowing.
//!
//! See `design.md` "Decision: Modifier-gated key consumption with FFI
//! normalization".

use crate::dispatch::{
    clamp_selected_to_view, focused_idx, reanchor_selected_to_focus, DispatchContext,
    DispatchState,
};
use crate::effect::Effect;
use crate::input::InputKey;
use crate::matcher::Matcher;
use crate::mode::Mode;

/// Filter-mode dispatch.
pub fn handle_filter_key(
    state: &mut DispatchState,
    ctx: &DispatchContext,
    key: InputKey,
) -> Vec<Effect> {
    match key {
        // ── Esc ───────────────────────────────────────────────────────────
        InputKey::Esc => {
            if !state.query.is_empty() {
                // Clear query; re-anchor selected to focused pane (matches
                // the post-condition of backspacing the query down to empty).
                state.query.clear();
                let f_idx = focused_idx(&state.panes, state.focused_pane_id);
                reanchor_selected_to_focus(state, f_idx);
                vec![Effect::Render]
            } else {
                // Empty query: drop to command mode.
                state.mode = Mode::Command;
                let f_idx = focused_idx(&state.panes, state.focused_pane_id);
                reanchor_selected_to_focus(state, f_idx);
                vec![Effect::Render]
            }
        }

        // ── Backspace ─────────────────────────────────────────────────────
        InputKey::Backspace => {
            if state.query.is_empty() {
                // Backspace on empty query is silent no-op.
                return Vec::new();
            }
            state.query.pop();
            if state.query.is_empty() {
                // Just emptied — re-anchor to focused pane (parity with
                // Esc-clear).
                let f_idx = focused_idx(&state.panes, state.focused_pane_id);
                reanchor_selected_to_focus(state, f_idx);
            } else {
                // Still non-empty: snap to top of filtered view (selected=0).
                // The shim recomputes filtered_indices on the next dispatch
                // and clamps; we just set selected = 0 here.
                state.selected = 0;
            }
            vec![Effect::Render]
        }

        // ── Arrow keys: nav within filtered view ──────────────────────────
        InputKey::ArrowDown => {
            let view_len = effective_view_len(state, ctx);
            if view_len < 2 {
                return Vec::new();
            }
            let new = state.selected.saturating_add(1);
            if new >= view_len {
                // Saturate at bottom; no wrap (filter UX matches fzf).
                return Vec::new();
            }
            state.selected = new;
            vec![Effect::Render]
        }
        InputKey::ArrowUp => {
            let view_len = effective_view_len(state, ctx);
            if view_len < 2 {
                return Vec::new();
            }
            if state.selected == 0 {
                return Vec::new();
            }
            state.selected -= 1;
            vec![Effect::Render]
        }

        // ── Enter: focus the currently-selected filtered match ────────────
        InputKey::Enter => {
            let target_idx = state.selected_pane_index(&ctx.filtered_indices);
            let Some(panes_idx) = target_idx else {
                return Vec::new();
            };
            let Some(pane) = state.panes.get(panes_idx).and_then(|opt| opt.as_ref()) else {
                return Vec::new();
            };
            // Close-before-FocusPane order is mandatory.
            vec![Effect::Close, Effect::FocusPane(pane.id)]
        }

        // ── Char branches ─────────────────────────────────────────────────
        InputKey::Char(c, modifiers) => {
            // Modifier gate: only plain (post-FFI-normalized) printables
            // append to the query. Modified inputs (Ctrl+W, Alt+a, etc.) are
            // declined explicitly with Noop so we don't silently swallow
            // standard terminal expectations.
            if !modifiers.is_plain() {
                return vec![Effect::Noop];
            }
            state.query.push(c);
            // Snap to top of filtered view on every query mutation. Shim
            // recomputes filtered_indices afterward and clamps in case the
            // view shrank to zero.
            state.selected = 0;
            // We can't recompute filtered_indices.len() here (no matcher
            // access in this signature); the shim post-clamps.
            let _ = clamp_selected_to_view; // kept in scope for callers
            vec![Effect::Render]
        }

        // Other keys not bound in filter mode.
        InputKey::Other => Vec::new(),
    }
}

/// Compute `filtered_indices` for the current state. Empty-query short-
/// circuit returns `0..panes.len()` filtered to Live entries (placeholders
/// excluded). Non-empty query runs the matcher against each Live pane's
/// display string and returns indices sorted by `(score DESC, panes_index ASC)`
/// — explicit tie-breaker for stable ordering across renders.
///
/// **Placeholder exclusion**: `state.panes` may contain `None` entries
/// (placeholders for unresolved bookmarks during the partial-restore window).
/// Filter mode operates on Live panes only — placeholders are never present
/// in `filtered_indices`.
///
/// See `specs/filter-mode/spec.md` "Fuzzy matching with configurable
/// algorithm".
pub fn filtered_indices<M: Matcher>(state: &DispatchState, matcher: &mut M) -> Vec<usize> {
    if state.query.is_empty() {
        // Empty query: all Live panes in original order.
        return state
            .panes
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| opt.as_ref().map(|_| i))
            .collect();
    }

    // Non-empty query: score each Live pane.
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (i, opt) in state.panes.iter().enumerate() {
        let Some(pane) = opt.as_ref() else { continue };
        let haystack = format!("{}", pane); // Pane Display = "tab | title"
        if let Some((score, _idx)) = matcher.match_indices(&haystack, &state.query) {
            scored.push((score, i));
        }
    }
    // Sort by (score DESC, panes_index ASC) — explicit tie-breaker for
    // render stability so `selected = 0` lands on the same pane on
    // consecutive renders with the same query.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// View-length helper for arrow nav. In filter mode with non-empty query,
/// uses `ctx.filtered_indices.len()`; otherwise falls back to
/// `state.panes.len()` (since arrow nav with empty query falls through to
/// the panes Vec directly).
fn effective_view_len(state: &DispatchState, ctx: &DispatchContext) -> usize {
    if matches!(state.mode, Mode::Filter) && !state.query.is_empty() {
        ctx.filtered_indices.len()
    } else {
        state.panes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ModifierSet;
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

    fn ck(c: char) -> InputKey {
        InputKey::Char(c, ModifierSet::PLAIN)
    }

    fn ck_mod(c: char, m: ModifierSet) -> InputKey {
        InputKey::Char(c, m)
    }

    fn ctrl() -> ModifierSet {
        ModifierSet {
            ctrl: true,
            ..Default::default()
        }
    }

    fn fresh_filter(panes: Vec<Option<Pane>>) -> DispatchState {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.panes = panes;
        s
    }

    fn empty_ctx() -> DispatchContext {
        DispatchContext::default()
    }

    // ── Char append ───────────────────────────────────────────────────────

    #[test]
    fn plain_letter_appends_to_query() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        let r = handle_filter_key(&mut s, &empty_ctx(), ck('e'));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.query, "e");
    }

    #[test]
    fn multi_chars_build_query() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        handle_filter_key(&mut s, &empty_ctx(), ck('e'));
        handle_filter_key(&mut s, &empty_ctx(), ck('d'));
        handle_filter_key(&mut s, &empty_ctx(), ck('i'));
        assert_eq!(s.query, "edi");
    }

    #[test]
    fn ctrl_w_in_filter_returns_noop_does_not_append() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        s.query = "ed".to_owned();
        let r = handle_filter_key(&mut s, &empty_ctx(), ck_mod('w', ctrl()));
        assert_eq!(r, vec![Effect::Noop]);
        assert_eq!(s.query, "ed", "Ctrl+W must not append");
    }

    #[test]
    fn ctrl_c_in_filter_returns_noop() {
        // Filter-mode Ctrl+c does NOT close (no carve-out for filter); shim
        // emits Noop. To close from filter, user presses Esc.
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        let r = handle_filter_key(&mut s, &empty_ctx(), ck_mod('c', ctrl()));
        assert_eq!(r, vec![Effect::Noop]);
    }

    #[test]
    fn first_keystroke_snaps_selected_to_zero() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x")); 3]);
        s.selected = 2;
        handle_filter_key(&mut s, &empty_ctx(), ck('e'));
        assert_eq!(s.selected, 0);
    }

    // ── Backspace ─────────────────────────────────────────────────────────

    #[test]
    fn backspace_pops_last_char() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        s.query = "edit".to_owned();
        let r = handle_filter_key(&mut s, &empty_ctx(), InputKey::Backspace);
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.query, "edi");
    }

    #[test]
    fn backspace_on_empty_query_returns_empty() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        s.query = String::new();
        let r = handle_filter_key(&mut s, &empty_ctx(), InputKey::Backspace);
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn backspace_to_empty_re_anchors_to_focused() {
        let mut s = fresh_filter(vec![
            Some(p(10, "a", "x")),
            Some(p(20, "b", "y")),
            Some(p(30, "c", "z")),
        ]);
        s.query = "e".to_owned();
        s.selected = 0;
        s.focused_pane_id = Some(20); // panes[1]
        handle_filter_key(&mut s, &empty_ctx(), InputKey::Backspace);
        assert_eq!(s.query, "");
        assert_eq!(s.selected, 1, "should re-anchor to focused pane index");
    }

    // ── Esc ───────────────────────────────────────────────────────────────

    #[test]
    fn esc_with_nonempty_query_clears_and_stays_in_filter() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        s.query = "ed".to_owned();
        let r = handle_filter_key(&mut s, &empty_ctx(), InputKey::Esc);
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.query, "");
        assert_eq!(s.mode, Mode::Filter, "Esc with non-empty query stays in filter");
    }

    #[test]
    fn esc_with_empty_query_drops_to_command() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        s.query = String::new();
        let r = handle_filter_key(&mut s, &empty_ctx(), InputKey::Esc);
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.mode, Mode::Command);
    }

    #[test]
    fn esc_clear_re_anchors_to_focused() {
        let mut s = fresh_filter(vec![Some(p(10, "a", "x")), Some(p(20, "b", "y"))]);
        s.query = "ed".to_owned();
        s.selected = 0;
        s.focused_pane_id = Some(20);
        handle_filter_key(&mut s, &empty_ctx(), InputKey::Esc);
        assert_eq!(s.query, "");
        assert_eq!(s.selected, 1);
    }

    // ── Arrow nav ─────────────────────────────────────────────────────────

    #[test]
    fn arrow_down_moves_selected_within_filtered_view() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x")); 3]);
        s.query = "x".to_owned();
        let mut ctx = empty_ctx();
        ctx.filtered_indices = vec![0, 1, 2];
        s.selected = 0;
        let r = handle_filter_key(&mut s, &ctx, InputKey::ArrowDown);
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.selected, 1);
    }

    #[test]
    fn arrow_up_moves_selected_within_filtered_view() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x")); 3]);
        s.query = "x".to_owned();
        let mut ctx = empty_ctx();
        ctx.filtered_indices = vec![0, 1, 2];
        s.selected = 2;
        let r = handle_filter_key(&mut s, &ctx, InputKey::ArrowUp);
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.selected, 1);
    }

    #[test]
    fn arrow_down_at_bottom_of_filtered_saturates() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x")); 3]);
        s.query = "x".to_owned();
        let mut ctx = empty_ctx();
        ctx.filtered_indices = vec![0, 1, 2];
        s.selected = 2;
        let r = handle_filter_key(&mut s, &ctx, InputKey::ArrowDown);
        assert_eq!(r, Vec::<Effect>::new());
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn arrow_up_at_top_saturates() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x")); 3]);
        s.query = "x".to_owned();
        let mut ctx = empty_ctx();
        ctx.filtered_indices = vec![0, 1, 2];
        s.selected = 0;
        let r = handle_filter_key(&mut s, &ctx, InputKey::ArrowUp);
        assert_eq!(r, Vec::<Effect>::new());
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn arrow_on_empty_view_returns_empty() {
        let mut s = fresh_filter(vec![]);
        let r = handle_filter_key(&mut s, &empty_ctx(), InputKey::ArrowDown);
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── Enter ─────────────────────────────────────────────────────────────

    #[test]
    fn enter_on_filtered_match_emits_close_then_focus() {
        let mut s = fresh_filter(vec![
            Some(p(10, "a", "x")),
            Some(p(20, "shell", "edit log")),
            Some(p(30, "c", "z")),
        ]);
        s.query = "ed".to_owned();
        let mut ctx = empty_ctx();
        ctx.filtered_indices = vec![1]; // panes[1] is the only match
        s.selected = 0;
        let r = handle_filter_key(&mut s, &ctx, InputKey::Enter);
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(20)]);
    }

    #[test]
    fn enter_on_empty_filtered_view_returns_empty() {
        let mut s = fresh_filter(vec![Some(p(1, "a", "x"))]);
        s.query = "zzz".to_owned();
        let mut ctx = empty_ctx();
        ctx.filtered_indices = Vec::new();
        let r = handle_filter_key(&mut s, &ctx, InputKey::Enter);
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn enter_with_empty_query_uses_panes_directly() {
        // Empty query → selected_pane_index falls through to command-like
        // path (panes index = selected). Enter focuses panes[selected].
        let mut s = fresh_filter(vec![Some(p(10, "a", "x")), Some(p(20, "b", "y"))]);
        s.query = String::new();
        s.selected = 1;
        let r = handle_filter_key(&mut s, &empty_ctx(), InputKey::Enter);
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(20)]);
    }

    // ── filtered_indices ──────────────────────────────────────────────────

    #[test]
    fn filtered_indices_empty_query_returns_all_live() {
        let s = {
            let mut s = DispatchState::default();
            s.mode = Mode::Filter;
            s.panes = vec![Some(p(1, "a", "x")), None, Some(p(2, "b", "y"))];
            s
        };
        let mut m = SubstringMatcher::new();
        let idx = filtered_indices(&s, &mut m);
        assert_eq!(idx, vec![0, 2], "placeholders excluded");
    }

    #[test]
    fn filtered_indices_substring_query() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.panes = vec![
            Some(p(1, "shell", "edit log")),
            Some(p(2, "build", "cargo")),
            Some(p(3, "work", "edit src")),
        ];
        s.query = "edit".to_owned();
        let mut m = SubstringMatcher::new();
        let idx = filtered_indices(&s, &mut m);
        // Both panes 0 and 2 contain "edit"; tie-broken by panes_index ASC.
        // SubstringMatcher scores -(start position), so "shell | edit log"
        // has start=8, "work | edit src" has start=7. score for pane[0] is
        // -8, for pane[2] is -7. -7 > -8, so pane[2] ranks first.
        assert_eq!(idx[0], 2);
        assert_eq!(idx[1], 0);
        assert!(!idx.contains(&1));
    }

    #[test]
    fn filtered_indices_excludes_placeholders() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.panes = vec![
            Some(p(1, "a", "edit")),
            None, // placeholder — must not be matched
            Some(p(2, "b", "edit too")),
        ];
        s.query = "edit".to_owned();
        let mut m = SubstringMatcher::new();
        let idx = filtered_indices(&s, &mut m);
        assert!(!idx.contains(&1));
        assert!(idx.contains(&0));
        assert!(idx.contains(&2));
    }

    #[test]
    fn filtered_indices_tie_breaker_smaller_panes_idx_first() {
        // Construct two panes with identical scores; verify smaller index
        // sorts first.
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.panes = vec![
            Some(p(1, "a", "edit")),
            Some(p(2, "b", "edit")),
        ];
        s.query = "edit".to_owned();
        let mut m = SubstringMatcher::new();
        let idx = filtered_indices(&s, &mut m);
        // Display strings: "a | edit" (start=4) and "b | edit" (start=4)
        // → identical scores. Tie-broken by panes_index ASC.
        assert_eq!(idx, vec![0, 1]);
    }
}
