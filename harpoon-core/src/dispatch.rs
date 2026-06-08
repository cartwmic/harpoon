//! Pure dispatch core. Per-mode handlers (in `command.rs`, `filter.rs`,
//! `jump.rs`) mutate `DispatchState` and read `DispatchContext`, returning
//! `Vec<Effect>`. The plugin shim translates effects into zellij FFI calls.
//!
//! See `design.md` "Decision: Pure dispatch core" and
//! `specs/mode-state-machine/spec.md`.

use crate::input::InputKey;
use crate::mode::Mode;
use crate::pane::Pane;

/// Persistent dispatch state. Owned by `harpoon-plugin::State`; passed by
/// `&mut` to handlers so they can mutate `mode`, `query`, `panes`, `selected`.
///
/// `panes` is a sparse `Vec<Option<Pane>>` during the partial-restore window:
/// `Some(p)` = live pane at this slot index; `None` = placeholder (the
/// matching `Persistence::bookmarks` entry has `index = Some(this_index)` but
/// hasn't yet been observed in any `PaneManifest`). On the first user
/// mutation that actually mutates (`a`/`A`/`d`/`K`/`J`), `freeze_on_user_mutation`
/// compacts `panes` to dense for the rest of the session.
///
/// See `design.md` "Decision: `state.panes` is a sparse `Vec<Option<Pane>>`".
#[derive(Debug, Clone, Default)]
pub struct DispatchState {
    /// Current interaction mode.
    pub mode: Mode,
    /// Mode reset target on `Effect::Close` and load.
    pub default_mode: Mode,
    /// Filter-mode query buffer. Empty in command/jump mode.
    pub query: String,
    /// Sparse pane Vec. See struct-level docs.
    pub panes: Vec<Option<Pane>>,
    /// Currently-highlighted row. In command/jump mode indexes `panes`
    /// directly (may land on a `None` placeholder, in which case mutation
    /// handlers no-op). In filter mode with non-empty query, indexes the
    /// `DispatchContext::filtered_indices` slice provided per-event.
    pub selected: usize,
    /// `id` of the currently-focused terminal pane, captured by the FFI
    /// shim from `PaneInfo.is_focused` events. Used by reanchor helpers.
    pub focused_pane_id: Option<u32>,
}

/// Event-local context the FFI shim builds per `Event::Key` and passes to
/// the dispatch loop. Carries everything handlers need that isn't part of
/// long-lived `DispatchState`.
#[derive(Debug, Clone, Default)]
pub struct DispatchContext {
    /// The user's currently-focused terminal pane (sticky per the existing
    /// fork's `da678cb` semantics).
    pub focused_pane: Option<Pane>,
    /// All panes currently visible in the zellij `PaneManifest`, projected
    /// into `harpoon-core::Pane`. Sorted by `(tab.position ASC, PaneInfo.id ASC)`
    /// per `design.md` "Decision: A add-all deterministic order".
    pub visible_panes: Vec<Pane>,
    /// Score-ordered filter view (with `(score DESC, panes_index ASC)`
    /// tie-breaker). Each entry is an index into `DispatchState::panes`.
    /// Built by the shim when the active mode is `Filter`; empty otherwise.
    /// Always references Live (`Some`) entries; placeholders excluded.
    pub filtered_indices: Vec<usize>,
}

impl DispatchState {
    /// Resolve the underlying `panes` index for the current `selected`,
    /// honoring filter mode's "selected indexes filtered_indices" semantics.
    ///
    /// - In command/jump mode: returns `Some(self.selected)` if it's in
    ///   range, else `None`.
    /// - In filter mode with non-empty query: returns
    ///   `Some(filtered_indices[self.selected])` if selected is in range, else
    ///   `None`.
    /// - In filter mode with empty query: same as command mode (the shim
    ///   should pass `&[]` for `filtered_indices` in this case, since the
    ///   filter is inactive).
    ///
    /// **Note**: this returns the panes-vec index, not the `Pane` itself.
    /// Callers must `panes[i].as_ref()` to get the live pane (or detect a
    /// `None` placeholder).
    pub fn selected_pane_index(&self, filtered_indices: &[usize]) -> Option<usize> {
        match self.mode {
            Mode::Filter if !self.query.is_empty() => filtered_indices.get(self.selected).copied(),
            _ => {
                if self.selected < self.panes.len() {
                    Some(self.selected)
                } else {
                    None
                }
            }
        }
    }
}

/// Re-anchor `selected` to the focused pane's index when the mode-aware gate
/// passes. Gate: `mode == Command || (mode == Filter && query.is_empty())`.
///
/// On gate-pass: `selected = focused_idx.unwrap_or(0).min(panes.len() - 1)`.
/// On gate-fail: `selected` is unchanged.
///
/// Used by `update_panes`, `close_helper`, and filter Esc-clear / backspace-
/// to-empty per `design.md` "Two-helper selection model".
pub fn reanchor_selected_to_focus(state: &mut DispatchState, focused_idx: Option<usize>) {
    let gate_pass = matches!(state.mode, Mode::Command)
        || (matches!(state.mode, Mode::Filter) && state.query.is_empty());
    if !gate_pass {
        return;
    }
    let target = focused_idx.unwrap_or(0);
    let max = state.panes.len().saturating_sub(1);
    state.selected = target.min(max);
}

/// Clamp `selected` against the current visible view length, regardless of
/// mode. Caller passes:
/// - `panes.len()` in command/jump mode
/// - `filtered_indices.len()` in filter mode with non-empty query
///
/// Sets `selected = selected.min(view_len.saturating_sub(1))`. View of
/// length 0 results in `selected = 0`.
pub fn clamp_selected_to_view(state: &mut DispatchState, view_len: usize) {
    state.selected = state.selected.min(view_len.saturating_sub(1));
}

/// Find the index `i` such that `panes[i] == Some(p)` and `p.id == id`.
/// Returns `None` if `id` is `None` or no live pane in `panes` has that id.
///
/// Single canonical helper for "where does the focused pane live in `panes`?"
/// — used by `update_panes`, `close_helper`, filter-mode Esc-clear, and the
/// backspace-to-empty re-anchor.
pub fn focused_idx(panes: &[Option<Pane>], id: Option<u32>) -> Option<usize> {
    let id = id?;
    panes.iter().position(|opt| match opt {
        Some(p) => p.id == id,
        None => false,
    })
}

/// Top-level dispatch entry point. Matches on `state.mode` and delegates to
/// the per-mode handler.
///
/// Filter and Jump handlers land in Phases 5 and 6; for now they return
/// `vec![]` so this dispatcher compiles and the structural shape is correct.
pub fn dispatch(
    state: &mut DispatchState,
    ctx: &DispatchContext,
    store: &mut crate::bookmark::BookmarkStore,
    key: InputKey,
) -> Vec<crate::effect::Effect> {
    match state.mode {
        Mode::Command => crate::command::handle_command_key(state, ctx, store, key),
        Mode::Filter => crate::filter::handle_filter_key(state, ctx, key),
        Mode::Jump => crate::jump::handle_jump_key(state, ctx, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ModifierSet;

    fn p(id: u32, tab: &str, title: &str) -> Pane {
        Pane {
            id,
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            tab_position: 0,
        }
    }

    // ---------- selected_pane_index ----------

    #[test]
    fn selected_pane_index_command_mode_in_range() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x")), Some(p(2, "b", "y"))];
        s.selected = 1;
        assert_eq!(s.selected_pane_index(&[]), Some(1));
    }

    #[test]
    fn selected_pane_index_command_out_of_range() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x"))];
        s.selected = 5;
        assert_eq!(s.selected_pane_index(&[]), None);
    }

    #[test]
    fn selected_pane_index_jump_mode_uses_panes_directly() {
        let mut s = DispatchState::default();
        s.mode = Mode::Jump;
        s.panes = vec![Some(p(1, "a", "x")), Some(p(2, "b", "y"))];
        s.selected = 0;
        assert_eq!(s.selected_pane_index(&[]), Some(0));
    }

    #[test]
    fn selected_pane_index_filter_empty_query_uses_panes() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.panes = vec![Some(p(1, "a", "x")), Some(p(2, "b", "y"))];
        s.selected = 1;
        // Empty query → falls through to command-like path.
        // Caller passes &[] for filtered_indices because filter is inactive.
        assert_eq!(s.selected_pane_index(&[]), Some(1));
    }

    #[test]
    fn selected_pane_index_filter_nonempty_query_uses_filtered() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "ed".to_owned();
        s.panes = vec![
            Some(p(1, "a", "x")),
            Some(p(2, "shell", "edit log")),
            Some(p(3, "build", "cargo")),
        ];
        // Filter view contains panes 1 and 2 in score order.
        let filtered = vec![1usize, 2];
        s.selected = 0;
        assert_eq!(s.selected_pane_index(&filtered), Some(1));
        s.selected = 1;
        assert_eq!(s.selected_pane_index(&filtered), Some(2));
        s.selected = 2;
        assert_eq!(s.selected_pane_index(&filtered), None);
    }

    #[test]
    fn selected_pane_index_filter_view_with_placeholder_present() {
        // panes[1] is a placeholder; the filter view excludes it.
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "x".to_owned();
        s.panes = vec![Some(p(1, "a", "x")), None, Some(p(3, "c", "x"))];
        let filtered = vec![0usize, 2];
        s.selected = 0;
        assert_eq!(s.selected_pane_index(&filtered), Some(0));
        s.selected = 1;
        assert_eq!(s.selected_pane_index(&filtered), Some(2));
    }

    // ---------- reanchor_selected_to_focus ----------

    #[test]
    fn reanchor_in_command_mode_sets_to_focused_idx() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x")); 5];
        s.selected = 0;
        reanchor_selected_to_focus(&mut s, Some(3));
        assert_eq!(s.selected, 3);
    }

    #[test]
    fn reanchor_in_command_mode_with_none_falls_back_to_zero() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x")); 5];
        s.selected = 4;
        reanchor_selected_to_focus(&mut s, None);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn reanchor_clamps_to_panes_len() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes = vec![Some(p(1, "a", "x")); 3];
        s.selected = 0;
        reanchor_selected_to_focus(&mut s, Some(99));
        assert_eq!(s.selected, 2); // panes.len() - 1
    }

    #[test]
    fn reanchor_in_filter_with_empty_query_passes_gate() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = String::new();
        s.panes = vec![Some(p(1, "a", "x")); 4];
        s.selected = 0;
        reanchor_selected_to_focus(&mut s, Some(2));
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn reanchor_in_filter_with_nonempty_query_does_not_change_selected() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        s.query = "ed".to_owned();
        s.panes = vec![Some(p(1, "a", "x")); 4];
        s.selected = 1;
        reanchor_selected_to_focus(&mut s, Some(3));
        assert_eq!(s.selected, 1, "filter+nonempty-query should NOT reanchor");
    }

    #[test]
    fn reanchor_in_jump_does_not_change_selected() {
        let mut s = DispatchState::default();
        s.mode = Mode::Jump;
        s.panes = vec![Some(p(1, "a", "x")); 4];
        s.selected = 2;
        reanchor_selected_to_focus(&mut s, Some(0));
        assert_eq!(s.selected, 2, "jump mode should NOT reanchor");
    }

    #[test]
    fn reanchor_with_empty_panes_lands_at_zero() {
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        s.panes.clear();
        s.selected = 7;
        reanchor_selected_to_focus(&mut s, Some(2));
        assert_eq!(s.selected, 0); // saturating_sub on len 0
    }

    // ---------- clamp_selected_to_view ----------

    #[test]
    fn clamp_selected_in_range_unchanged() {
        let mut s = DispatchState::default();
        s.selected = 2;
        clamp_selected_to_view(&mut s, 5);
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn clamp_selected_above_view_clamps() {
        let mut s = DispatchState::default();
        s.selected = 10;
        clamp_selected_to_view(&mut s, 3);
        assert_eq!(s.selected, 2); // view_len - 1
    }

    #[test]
    fn clamp_selected_view_zero_results_in_zero() {
        let mut s = DispatchState::default();
        s.selected = 5;
        clamp_selected_to_view(&mut s, 0);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn clamp_selected_at_boundary() {
        let mut s = DispatchState::default();
        s.selected = 4;
        clamp_selected_to_view(&mut s, 5);
        assert_eq!(s.selected, 4); // view_len - 1
    }

    // ---------- focused_idx ----------

    #[test]
    fn focused_idx_finds_live_pane() {
        let panes = vec![Some(p(1, "a", "x")), Some(p(7, "b", "y")), Some(p(3, "c", "z"))];
        assert_eq!(focused_idx(&panes, Some(7)), Some(1));
        assert_eq!(focused_idx(&panes, Some(3)), Some(2));
        assert_eq!(focused_idx(&panes, Some(1)), Some(0));
    }

    #[test]
    fn focused_idx_skips_placeholders() {
        let panes = vec![Some(p(1, "a", "x")), None, Some(p(3, "c", "z"))];
        assert_eq!(focused_idx(&panes, Some(3)), Some(2));
        assert_eq!(focused_idx(&panes, Some(1)), Some(0));
    }

    #[test]
    fn focused_idx_id_not_present_returns_none() {
        let panes = vec![Some(p(1, "a", "x")), Some(p(2, "b", "y"))];
        assert_eq!(focused_idx(&panes, Some(99)), None);
    }

    #[test]
    fn focused_idx_none_id_returns_none() {
        let panes = vec![Some(p(1, "a", "x"))];
        assert_eq!(focused_idx(&panes, None), None);
    }

    #[test]
    fn focused_idx_empty_panes() {
        let panes: Vec<Option<Pane>> = Vec::new();
        assert_eq!(focused_idx(&panes, Some(1)), None);
    }

    // ---------- dispatch (skeleton) ----------

    #[test]
    fn dispatch_unbound_in_command_returns_empty() {
        // Letter `b` is unbound in command mode (not a digit slot, not a
        // command key). Handler returns vec![] per command.rs contract.
        let mut s = DispatchState::default();
        s.mode = Mode::Command;
        let ctx = DispatchContext::default();
        let mut store = crate::bookmark::BookmarkStore::default();
        let effects = dispatch(&mut s, &ctx, &mut store, InputKey::Char('b', ModifierSet::PLAIN));
        assert!(effects.is_empty());
    }

    #[test]
    fn dispatch_filter_mode_appends_to_query() {
        let mut s = DispatchState::default();
        s.mode = Mode::Filter;
        let ctx = DispatchContext::default();
        let mut store = crate::bookmark::BookmarkStore::default();
        let effects = dispatch(&mut s, &ctx, &mut store, InputKey::Char('a', ModifierSet::PLAIN));
        assert_eq!(effects, vec![crate::effect::Effect::Render]);
        assert_eq!(s.query, "a");
    }

    #[test]
    fn dispatch_jump_mode_with_no_panes_returns_empty() {
        let mut s = DispatchState::default();
        s.mode = Mode::Jump;
        let ctx = DispatchContext::default();
        let mut store = crate::bookmark::BookmarkStore::default();
        let effects = dispatch(&mut s, &ctx, &mut store, InputKey::Char('1', ModifierSet::PLAIN));
        assert!(effects.is_empty());
    }

    #[test]
    fn dispatch_jump_mode_with_live_pane_emits_close_focus() {
        use crate::pane::Pane;
        let mut s = DispatchState::default();
        s.mode = Mode::Jump;
        s.panes = vec![Some(Pane {
            id: 7,
            tab_name: "t".into(),
            pane_title: "x".into(),
            tab_position: 0,
        })];
        let ctx = DispatchContext::default();
        let mut store = crate::bookmark::BookmarkStore::default();
        let effects = dispatch(&mut s, &ctx, &mut store, InputKey::Char('1', ModifierSet::PLAIN));
        assert_eq!(
            effects,
            vec![
                crate::effect::Effect::Close,
                crate::effect::Effect::FocusPane(7)
            ]
        );
    }
}
