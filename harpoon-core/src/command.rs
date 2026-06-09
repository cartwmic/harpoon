//! Command-mode key dispatch.
//!
//! Today's bare-key behavior: `a`/`A` add, `d` delete, `j`/`k` nav (wrapping),
//! `K`/`J` reorder (saturating), `l`/`Enter` focus, `c`/`Esc` close, `1-9`
//! slot jump, `/` enter filter, `#` enter jump.
//!
//! See `specs/mode-state-machine/spec.md` and `specs/reorder/spec.md` for the
//! full contract; `tasks.md` 4.1 enumerates each branch's effects.
//!
//! **Modifier gating** (post-FFI normalization):
//! - ASCII letters and digits gate on `modifiers.is_plain()`. Modified
//!   inputs return `vec![Effect::Noop]` EXCEPT for `c` (close), which
//!   accepts any modifier set so today's accidental `Ctrl+c` muscle memory
//!   keeps working.
//! - Symbol keys (`/`, `#`) accept any modifier set since keyboard layouts
//!   vary on whether `#` arrives with implicit Shift.
//!
//! See `design.md` "Decision: Modifier-gated key consumption with FFI
//! normalization".

use crate::bookmark::{BookmarkStore, PaneBookmark};
use crate::dispatch::{DispatchContext, DispatchState};
use crate::effect::Effect;
use crate::freeze::freeze_on_user_mutation;
use crate::input::InputKey;
use crate::mode::Mode;
use crate::pane::Pane;
use crate::slot::slot_index_from_char;

/// Top-level command-mode handler. Per `tasks.md` task 4.1, each branch
/// enumerates its `Vec<Effect>` explicitly. See module-level docs for the
/// modifier-gating policy.
pub fn handle_command_key(
    state: &mut DispatchState,
    ctx: &DispatchContext,
    store: &mut BookmarkStore,
    key: InputKey,
) -> Vec<Effect> {
    match key {
        // ── Esc closes ────────────────────────────────────────────────────
        InputKey::Esc => vec![Effect::Close],

        // ── Enter / l → focus selected ────────────────────────────────────
        InputKey::Enter => focus_selected(state),

        // ── Char branches ─────────────────────────────────────────────────
        InputKey::Char(c, modifiers) => {
            // `c` (close) is the special-case carve-out: ANY modifier set
            // accepts. Preserves today's accidental `Ctrl+c` close behavior.
            if c == 'c' {
                return vec![Effect::Close];
            }

            // Symbol keys accept any modifier set (keyboard-layout robustness).
            if c == '/' {
                state.mode = Mode::Filter;
                return vec![Effect::Render];
            }
            if c == '#' {
                state.mode = Mode::Jump;
                return vec![Effect::Render];
            }

            // Letter/digit keys gate on `modifiers.is_plain()`. Modified
            // inputs (Ctrl+a, Alt+d, etc.) return Noop — declines mutation
            // explicitly rather than silently swallowing.
            if !modifiers.is_plain() {
                return vec![Effect::Noop];
            }

            // Plain `c` is handled above (any-modifier accept); `/` and `#`
            // too. The remaining branches are all plain-only.
            match c {
                'l' => focus_selected(state),
                'a' => add_focused(state, ctx, store),
                'A' => add_all(state, ctx, store),
                'd' => delete_selected(state, store),
                'j' => nav_down(state),
                'k' => nav_up(state),
                'K' => reorder_up(state, store),
                'J' => reorder_down(state, store),
                _ => {
                    // Digit slot jump (`'1'..='9'`).
                    if let Some(slot_idx) = slot_index_from_char(c) {
                        // Letters map to slot indices 9..34 too, but command
                        // mode does NOT support letter slot jumps (per
                        // `specs/jump-mode/spec.md` "Digit-only jumps from
                        // command mode"). Reject letters here.
                        if c.is_ascii_lowercase() {
                            return Vec::new();
                        }
                        return jump_to_slot(state, slot_idx);
                    }
                    Vec::new()
                }
            }
        }

        // Backspace, ArrowUp, ArrowDown, Other — not bound in command mode.
        InputKey::Backspace | InputKey::ArrowUp | InputKey::ArrowDown | InputKey::Other => {
            Vec::new()
        }
    }
}

// ── Branch implementations ────────────────────────────────────────────────────

/// `Enter` / `Char('l', plain)`: focus the selected pane and close.
/// Empty-list and placeholder guards: returns `vec![]` if the selection is
/// out of range or on a `None` placeholder. Else emits `[Close, FocusPane(id)]`
/// in that order (Close BEFORE FocusPane is mandatory per
/// `design.md` "Decision: Pure dispatch core").
fn focus_selected(state: &DispatchState) -> Vec<Effect> {
    if state.panes.is_empty() || state.selected >= state.panes.len() {
        return Vec::new();
    }
    let Some(pane) = state.panes[state.selected].as_ref() else {
        // Placeholder selection — silently no-op so the user can move off it.
        return Vec::new();
    };
    vec![Effect::Close, Effect::FocusPane(pane.id)]
}

/// `Char('a', plain)` add focused: pin the user's currently-focused terminal
/// pane. Skips if no focused pane or already pinned. Triggers freeze before
/// mutating to compact the sparse Vec.
fn add_focused(
    state: &mut DispatchState,
    ctx: &DispatchContext,
    store: &mut BookmarkStore,
) -> Vec<Effect> {
    let Some(focused) = ctx.focused_pane.clone() else {
        return Vec::new();
    };
    if store.pane_id_to_bookmark_idx.contains_key(&focused.id) {
        // Already pinned.
        return Vec::new();
    }
    freeze_on_user_mutation(state, store);
    push_pane(state, store, focused);
    vec![Effect::Save, Effect::Render, Effect::Close]
}

/// `Char('A', plain)` add all: pin every visible pane not yet in the store.
/// Iterates `ctx.visible_panes` (the FFI shim's contract is that it's already
/// sorted by `tab.position ASC, PaneInfo.id ASC` per `design.md` "Decision:
/// A add-all deterministic order"). Returns `[Save, Render]` — **NO Close**;
/// `A` keeps the plugin open so the user can immediately reorder/jump per
/// `specs/mode-state-machine/spec.md` "A (add all) does not close the plugin".
fn add_all(
    state: &mut DispatchState,
    ctx: &DispatchContext,
    store: &mut BookmarkStore,
) -> Vec<Effect> {
    let new_panes: Vec<Pane> = ctx
        .visible_panes
        .iter()
        .filter(|p| !store.pane_id_to_bookmark_idx.contains_key(&p.id))
        .cloned()
        .collect();
    if new_panes.is_empty() {
        return Vec::new();
    }
    freeze_on_user_mutation(state, store);
    for pane in new_panes {
        push_pane(state, store, pane);
    }
    vec![Effect::Save, Effect::Render]
}

/// Push a single Live pane onto `state.panes` and create the matching
/// `PaneBookmark` + map entry. Caller must have already called freeze if
/// appropriate.
fn push_pane(state: &mut DispatchState, store: &mut BookmarkStore, pane: Pane) {
    let new_idx = state.panes.len();
    let bk_idx = store.bookmarks.len();
    store.bookmarks.push(PaneBookmark {
        tab_name: pane.tab_name.clone(),
        pane_title: pane.pane_title.clone(),
        index: Some(new_idx as u16),
        id: Some(pane.id),
    });
    store.pane_id_to_bookmark_idx.insert(pane.id, bk_idx);
    state.panes.push(Some(pane));
}

/// `Char('d', plain)` delete selected: only operates on a Live target. Freeze
/// first (which compacts panes; selected re-anchors to same pane id), then
/// remove the pane + bookmark + map entry, decrement subsequent bookmark
/// indices, rebuild map entries that pointed past the removed bookmark.
fn delete_selected(state: &mut DispatchState, store: &mut BookmarkStore) -> Vec<Effect> {
    if state.selected >= state.panes.len() {
        return Vec::new();
    }
    if state.panes[state.selected].is_none() {
        // Placeholder — silently no-op.
        return Vec::new();
    }

    freeze_on_user_mutation(state, store);

    // Post-freeze, panes is dense; selected may have moved. Recompute.
    if state.selected >= state.panes.len() || state.panes[state.selected].is_none() {
        // Defensive: freeze should have re-anchored us to a Live slot.
        return Vec::new();
    }
    let removed_pane = state.panes[state.selected].as_ref().unwrap().clone();
    let removed_panes_idx = state.selected;
    let Some(&removed_bk_idx) = store.pane_id_to_bookmark_idx.get(&removed_pane.id) else {
        // Should not happen post-freeze.
        return Vec::new();
    };

    // Remove the pane.
    state.panes.remove(removed_panes_idx);
    // Remove the bookmark.
    store.bookmarks.remove(removed_bk_idx);
    // Remove the map entry for this pane.
    store.pane_id_to_bookmark_idx.remove(&removed_pane.id);
    // Shift remaining map values: any bookmark idx > removed_bk_idx is now
    // one less.
    for v in store.pane_id_to_bookmark_idx.values_mut() {
        if *v > removed_bk_idx {
            *v -= 1;
        }
    }
    // Decrement bookmark `index` fields > removed_panes_idx.
    for b in &mut store.bookmarks {
        if let Some(i) = b.index {
            if (i as usize) > removed_panes_idx {
                b.index = Some(i - 1);
            }
        }
    }
    // Clamp selected against new view length.
    state.selected = state.selected.min(state.panes.len().saturating_sub(1));

    vec![Effect::Save, Effect::Render]
}

/// `j` (down): wrap from `panes.len() - 1` to `0`. Empty/single-element →
/// `vec![]`. May land on `None` placeholders without skipping.
fn nav_down(state: &mut DispatchState) -> Vec<Effect> {
    let len = state.panes.len();
    if len < 2 {
        return Vec::new();
    }
    state.selected = if state.selected >= len - 1 {
        0
    } else {
        state.selected + 1
    };
    vec![Effect::Render]
}

/// `k` (up): wrap from `0` to `panes.len() - 1`. Mirror of `nav_down`.
fn nav_up(state: &mut DispatchState) -> Vec<Effect> {
    let len = state.panes.len();
    if len < 2 {
        return Vec::new();
    }
    state.selected = if state.selected == 0 {
        len - 1
    } else {
        state.selected - 1
    };
    vec![Effect::Render]
}

/// `K` (shift-k): swap with previous. Pre-conditions: `panes.len() >= 2`,
/// `selected > 0`, BOTH `panes[selected]` and `panes[selected - 1]` Some.
/// Saturates at top (no wrap). Triggers freeze before swapping; swaps the
/// two affected bookmarks' `index` fields via the id→bookmark_idx map.
fn reorder_up(state: &mut DispatchState, store: &mut BookmarkStore) -> Vec<Effect> {
    if state.panes.len() < 2 || state.selected == 0 || state.selected >= state.panes.len() {
        return Vec::new();
    }
    if state.panes[state.selected].is_none() || state.panes[state.selected - 1].is_none() {
        return Vec::new();
    }

    freeze_on_user_mutation(state, store);

    // Post-freeze: panes dense; selected may have moved with the live pane id.
    if state.panes.len() < 2 || state.selected == 0 || state.selected >= state.panes.len() {
        return Vec::new();
    }
    swap_adjacent_panes_and_bookmarks(state, store, state.selected, state.selected - 1);
    state.selected -= 1;
    vec![Effect::Save, Effect::Render]
}

/// `J` (shift-j): mirror of `K`. Pre-conditions: `selected < panes.len() - 1`,
/// both targets Some.
fn reorder_down(state: &mut DispatchState, store: &mut BookmarkStore) -> Vec<Effect> {
    if state.panes.len() < 2 || state.selected >= state.panes.len() - 1 {
        return Vec::new();
    }
    if state.panes[state.selected].is_none() || state.panes[state.selected + 1].is_none() {
        return Vec::new();
    }

    freeze_on_user_mutation(state, store);

    if state.panes.len() < 2 || state.selected >= state.panes.len() - 1 {
        return Vec::new();
    }
    swap_adjacent_panes_and_bookmarks(state, store, state.selected, state.selected + 1);
    state.selected += 1;
    vec![Effect::Save, Effect::Render]
}

/// Swap `state.panes[a]` and `state.panes[b]` AND swap the corresponding
/// bookmarks' `index` fields. Both panes must be `Some` (caller-checked).
fn swap_adjacent_panes_and_bookmarks(
    state: &mut DispatchState,
    store: &mut BookmarkStore,
    a: usize,
    b: usize,
) {
    let id_a = state.panes[a].as_ref().unwrap().id;
    let id_b = state.panes[b].as_ref().unwrap().id;
    state.panes.swap(a, b);
    if let (Some(&bk_a), Some(&bk_b)) = (
        store.pane_id_to_bookmark_idx.get(&id_a),
        store.pane_id_to_bookmark_idx.get(&id_b),
    ) {
        // Swap the `index` fields. The bookmark Vec positions don't move;
        // only the `index` field on each bookmark updates to reflect the
        // new pane position.
        let idx_a = store.bookmarks[bk_a].index;
        let idx_b = store.bookmarks[bk_b].index;
        store.bookmarks[bk_a].index = idx_b;
        store.bookmarks[bk_b].index = idx_a;
    }
}

/// Digit slot jump from command mode (`'1'..='9'`). Only operates on Live
/// slots; placeholder slots (None) and OOB return `vec![]` so the saved-
/// position contract holds during partial restore.
fn jump_to_slot(state: &DispatchState, slot_idx: usize) -> Vec<Effect> {
    match state.panes.get(slot_idx).and_then(|opt| opt.as_ref()) {
        Some(pane) => vec![Effect::Close, Effect::FocusPane(pane.id)],
        None => Vec::new(),
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

    fn bm(tab: &str, title: &str, index: Option<u16>) -> PaneBookmark {
        PaneBookmark {
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            index,
            id: None,
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

    fn alt() -> ModifierSet {
        ModifierSet {
            alt: true,
            ..Default::default()
        }
    }

    fn shift() -> ModifierSet {
        ModifierSet {
            shift: true,
            ..Default::default()
        }
    }

    /// Build a fresh state + store with `panes` at the given slots and
    /// matching bookmarks. Each Live pane in `panes` gets a corresponding
    /// bookmark with `index = Some(its_position_in_panes)` and a map entry.
    /// Unresolved bookmarks for `None` slots may be added by callers as needed.
    fn fresh(panes: Vec<Option<Pane>>) -> (DispatchState, BookmarkStore) {
        let mut state = DispatchState::default();
        let mut store = BookmarkStore::default();
        for (i, opt) in panes.iter().enumerate() {
            if let Some(pane) = opt {
                let bk_idx = store.bookmarks.len();
                store.bookmarks.push(PaneBookmark {
                    tab_name: pane.tab_name.clone(),
                    pane_title: pane.pane_title.clone(),
                    index: Some(i as u16),
                    id: None,
                });
                store.pane_id_to_bookmark_idx.insert(pane.id, bk_idx);
            }
        }
        state.panes = panes;
        (state, store)
    }

    fn empty_ctx() -> DispatchContext {
        DispatchContext::default()
    }

    // ── Esc / c (close) ────────────────────────────────────────────────────

    #[test]
    fn esc_returns_close() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, InputKey::Esc);
        assert_eq!(r, vec![Effect::Close]);
    }

    #[test]
    fn plain_c_returns_close() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('c'));
        assert_eq!(r, vec![Effect::Close]);
    }

    #[test]
    fn ctrl_c_returns_close_carve_out() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck_mod('c', ctrl()));
        assert_eq!(r, vec![Effect::Close], "Ctrl+c carve-out preserves today's accidental close");
    }

    #[test]
    fn alt_c_returns_close_carve_out() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck_mod('c', alt()));
        assert_eq!(r, vec![Effect::Close]);
    }

    // ── Modifier gating on letters ────────────────────────────────────────

    #[test]
    fn ctrl_a_returns_noop() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck_mod('a', ctrl()));
        assert_eq!(r, vec![Effect::Noop]);
    }

    #[test]
    fn alt_a_returns_noop() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck_mod('a', alt()));
        assert_eq!(r, vec![Effect::Noop]);
    }

    #[test]
    fn ctrl_d_returns_noop() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n"))]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck_mod('d', ctrl()));
        assert_eq!(r, vec![Effect::Noop]);
    }

    // ── Symbol keys (any modifier accepted) ───────────────────────────────

    #[test]
    fn slash_enters_filter_mode_plain() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('/'));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.mode, Mode::Filter);
    }

    #[test]
    fn slash_enters_filter_mode_with_shift() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck_mod('/', shift()));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.mode, Mode::Filter);
    }

    #[test]
    fn hash_enters_jump_mode() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('#'));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.mode, Mode::Jump);
    }

    #[test]
    fn hash_enters_jump_mode_with_shift() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck_mod('#', shift()));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.mode, Mode::Jump);
    }

    // ── Enter / l ─────────────────────────────────────────────────────────

    #[test]
    fn enter_on_live_emits_close_then_focus() {
        let (mut s, mut store) = fresh(vec![Some(p(7, "w", "n"))]);
        s.selected = 0;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, InputKey::Enter);
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(7)]);
    }

    #[test]
    fn l_on_live_emits_close_then_focus() {
        let (mut s, mut store) = fresh(vec![Some(p(7, "w", "n"))]);
        s.selected = 0;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('l'));
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(7)]);
    }

    #[test]
    fn enter_on_empty_returns_empty() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, InputKey::Enter);
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn enter_on_placeholder_returns_empty() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n")), None]);
        s.selected = 1;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, InputKey::Enter);
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── Digit slot jumps ──────────────────────────────────────────────────

    #[test]
    fn digit_1_on_live_returns_close_then_focus() {
        let (mut s, mut store) = fresh(vec![Some(p(10, "w", "n")), Some(p(20, "s", "b"))]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('1'));
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(10)]);
    }

    #[test]
    fn digit_2_on_placeholder_returns_empty() {
        let (mut s, mut store) = fresh(vec![Some(p(10, "w", "n")), None]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('2'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn digit_5_on_short_list_returns_empty() {
        let (mut s, mut store) = fresh(vec![Some(p(10, "w", "n"))]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('5'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn letter_b_in_command_mode_returns_empty() {
        // `b` would be slot 10 in jump mode but command mode rejects letters.
        let panes: Vec<Option<Pane>> = (0..11).map(|i| Some(p(i + 1, "t", "x"))).collect();
        let (mut s, mut store) = fresh(panes);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('b'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── j/k wrap navigation ───────────────────────────────────────────────

    #[test]
    fn j_wraps_from_bottom_to_top() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n")); 4].into_iter().enumerate().map(|(i, _)| Some(p(i as u32 + 1, "w", "n"))).collect());
        s.selected = 3;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('j'));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn k_wraps_from_top_to_bottom() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n")); 4].into_iter().enumerate().map(|(i, _)| Some(p(i as u32 + 1, "w", "n"))).collect());
        s.selected = 0;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('k'));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.selected, 3);
    }

    #[test]
    fn j_on_empty_returns_empty() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('j'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn j_on_single_element_returns_empty() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n"))]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('j'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn k_on_single_element_returns_empty() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n"))]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('k'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn j_lands_on_placeholder() {
        // Nav doesn't skip None slots.
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n")), None, Some(p(2, "x", "y"))]);
        s.selected = 0;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('j'));
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.selected, 1, "should land on the None placeholder, not skip");
    }

    // ── K/J reorder ───────────────────────────────────────────────────────

    #[test]
    fn capital_k_at_top_returns_empty_no_freeze() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n")), Some(p(2, "x", "y"))]);
        s.selected = 0;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('K'));
        assert_eq!(r, Vec::<Effect>::new());
        assert!(!store.frozen, "K saturating no-op must not freeze");
    }

    #[test]
    fn capital_j_at_bottom_returns_empty_no_freeze() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n")), Some(p(2, "x", "y"))]);
        s.selected = 1;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('J'));
        assert_eq!(r, Vec::<Effect>::new());
        assert!(!store.frozen);
    }

    #[test]
    fn capital_k_on_empty_list_returns_empty() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('K'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn capital_k_on_single_element_returns_empty() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n"))]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('K'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn capital_k_neighbor_placeholder_returns_empty_no_freeze() {
        // [Some(P0), None, Some(P2)], selected=2. K would swap P2 with None — rejected.
        let p0 = p(10, "work", "nvim");
        let p2 = p(12, "build", "cargo");
        // fresh helper auto-builds bookmarks for Some slots; we need to add a bookmark
        // for the None slot (unresolved) too so the freeze logic is exercised.
        let mut s = DispatchState::default();
        let mut store = BookmarkStore::default();
        store.bookmarks.push(bm("work", "nvim", Some(0)));
        store.bookmarks.push(bm("shell", "edit", Some(1))); // unresolved
        store.bookmarks.push(bm("build", "cargo", Some(2)));
        store.pane_id_to_bookmark_idx.insert(10, 0);
        store.pane_id_to_bookmark_idx.insert(12, 2);
        s.panes = vec![Some(p0), None, Some(p2)];
        s.selected = 2;

        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('K'));
        assert_eq!(r, Vec::<Effect>::new());
        assert!(!store.frozen, "K with placeholder neighbor must not freeze");
        // panes unchanged.
        assert_eq!(s.panes.len(), 3);
        assert!(s.panes[1].is_none());
    }

    #[test]
    fn capital_k_on_live_neighbors_freezes_swaps_emits_save_render() {
        // Pre-freeze: [Some(P0), None, Some(P2)] selected=2, with B0/B1(unresolved)/B2.
        // K with selected=2: pre-condition checks BOTH panes[2] (Some) and panes[1] (None).
        // panes[1] is None, so this should be rejected like the test above.
        // To exercise the freeze + swap path, use a setup where neighbors are both Some.
        let p0 = p(10, "work", "nvim");
        let p1 = p(11, "shell", "bash");
        let p2 = p(12, "build", "cargo");
        let mut s = DispatchState::default();
        let mut store = BookmarkStore::default();
        store.bookmarks.push(bm("work", "nvim", Some(0)));
        store.bookmarks.push(bm("shell", "bash", Some(1)));
        store.bookmarks.push(bm("build", "cargo", Some(2)));
        store.pane_id_to_bookmark_idx.insert(10, 0);
        store.pane_id_to_bookmark_idx.insert(11, 1);
        store.pane_id_to_bookmark_idx.insert(12, 2);
        s.panes = vec![Some(p0), Some(p1), Some(p2)];
        s.selected = 2;

        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('K'));
        assert_eq!(r, vec![Effect::Save, Effect::Render]);
        assert!(store.frozen);
        // After K with selected=2: P2 swaps with P1.
        assert_eq!(s.panes[1].as_ref().unwrap().id, 12);
        assert_eq!(s.panes[2].as_ref().unwrap().id, 11);
        assert_eq!(s.selected, 1);
        // Bookmark indices on swapped panes flipped.
        assert_eq!(store.bookmarks[1].index, Some(2)); // B1 (P1) now at panes[2]
        assert_eq!(store.bookmarks[2].index, Some(1)); // B2 (P2) now at panes[1]
        assert_eq!(store.bookmarks[0].index, Some(0)); // B0 unchanged
    }

    #[test]
    fn capital_j_on_live_neighbors_freezes_swaps() {
        let p0 = p(10, "w", "n");
        let p1 = p(11, "x", "y");
        let mut s = DispatchState::default();
        let mut store = BookmarkStore::default();
        store.bookmarks.push(bm("w", "n", Some(0)));
        store.bookmarks.push(bm("x", "y", Some(1)));
        store.pane_id_to_bookmark_idx.insert(10, 0);
        store.pane_id_to_bookmark_idx.insert(11, 1);
        s.panes = vec![Some(p0), Some(p1)];
        s.selected = 0;

        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('J'));
        assert_eq!(r, vec![Effect::Save, Effect::Render]);
        assert_eq!(s.panes[0].as_ref().unwrap().id, 11);
        assert_eq!(s.panes[1].as_ref().unwrap().id, 10);
        assert_eq!(s.selected, 1);
        assert_eq!(store.bookmarks[0].index, Some(1));
        assert_eq!(store.bookmarks[1].index, Some(0));
    }

    // ── d delete ──────────────────────────────────────────────────────────

    #[test]
    fn d_on_live_freezes_removes_pane_and_bookmark() {
        let p0 = p(10, "w", "n");
        let p1 = p(11, "x", "y");
        let (mut s, mut store) = fresh(vec![Some(p0), Some(p1.clone())]);
        s.selected = 0;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('d'));
        assert_eq!(r, vec![Effect::Save, Effect::Render]);
        assert_eq!(s.panes, vec![Some(p1)]);
        assert_eq!(store.bookmarks.len(), 1);
        assert_eq!(store.bookmarks[0].index, Some(0));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&11), Some(&0));
        assert!(!store.pane_id_to_bookmark_idx.contains_key(&10));
    }

    #[test]
    fn d_on_placeholder_returns_empty_no_freeze() {
        let mut s = DispatchState::default();
        let mut store = BookmarkStore::default();
        store.bookmarks.push(bm("w", "n", Some(0)));
        store.bookmarks.push(bm("x", "y", Some(1))); // unresolved
        store.pane_id_to_bookmark_idx.insert(10, 0);
        s.panes = vec![Some(p(10, "w", "n")), None];
        s.selected = 1;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('d'));
        assert_eq!(r, Vec::<Effect>::new());
        assert!(!store.frozen);
        assert_eq!(s.panes.len(), 2);
    }

    #[test]
    fn d_on_empty_returns_empty() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('d'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn d_at_oob_selected_returns_empty() {
        let (mut s, mut store) = fresh(vec![Some(p(1, "w", "n"))]);
        s.selected = 5;
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('d'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── a add focused ─────────────────────────────────────────────────────

    #[test]
    fn a_add_focused_when_not_pinned() {
        let (mut s, mut store) = fresh(vec![]);
        let mut ctx = empty_ctx();
        ctx.focused_pane = Some(p(99, "work", "nvim"));
        let r = handle_command_key(&mut s, &ctx, &mut store, ck('a'));
        assert_eq!(r, vec![Effect::Save, Effect::Render, Effect::Close]);
        assert_eq!(s.panes.len(), 1);
        assert_eq!(s.panes[0].as_ref().unwrap().id, 99);
        assert_eq!(store.bookmarks.len(), 1);
        assert_eq!(store.bookmarks[0].index, Some(0));
        assert_eq!(store.pane_id_to_bookmark_idx.get(&99), Some(&0));
    }

    #[test]
    fn a_no_focused_pane_returns_empty() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('a'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn a_already_pinned_returns_empty() {
        let p0 = p(10, "w", "n");
        let (mut s, mut store) = fresh(vec![Some(p0.clone())]);
        let mut ctx = empty_ctx();
        ctx.focused_pane = Some(p0);
        let r = handle_command_key(&mut s, &ctx, &mut store, ck('a'));
        assert_eq!(r, Vec::<Effect>::new(), "already-pinned focused pane must not double-add");
        assert_eq!(s.panes.len(), 1);
    }

    // ── A add all ─────────────────────────────────────────────────────────

    #[test]
    fn capital_a_no_visible_returns_empty() {
        let (mut s, mut store) = fresh(vec![]);
        let r = handle_command_key(&mut s, &empty_ctx(), &mut store, ck('A'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn capital_a_all_already_pinned_returns_empty() {
        let p0 = p(10, "w", "n");
        let (mut s, mut store) = fresh(vec![Some(p0.clone())]);
        let mut ctx = empty_ctx();
        ctx.visible_panes = vec![p0];
        let r = handle_command_key(&mut s, &ctx, &mut store, ck('A'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn capital_a_emits_save_render_no_close_and_appends_in_input_order() {
        let p1 = p(10, "tab1", "a");
        let p2 = p(11, "tab1", "b");
        let p3 = p(12, "tab2", "c");
        let (mut s, mut store) = fresh(vec![]);
        let mut ctx = empty_ctx();
        // FFI shim contract: visible_panes already sorted (tab.position ASC, id ASC).
        ctx.visible_panes = vec![p1.clone(), p2.clone(), p3.clone()];
        let r = handle_command_key(&mut s, &ctx, &mut store, ck('A'));
        // CRITICAL: A does NOT close.
        assert_eq!(r, vec![Effect::Save, Effect::Render]);
        assert_eq!(s.panes.len(), 3);
        assert_eq!(s.panes[0].as_ref().unwrap().id, 10);
        assert_eq!(s.panes[1].as_ref().unwrap().id, 11);
        assert_eq!(s.panes[2].as_ref().unwrap().id, 12);
        assert_eq!(store.bookmarks.len(), 3);
    }

    #[test]
    fn capital_a_skips_already_pinned() {
        let p1 = p(10, "tab1", "a");
        let p2 = p(11, "tab1", "b");
        // p1 already pinned.
        let (mut s, mut store) = fresh(vec![Some(p1.clone())]);
        let mut ctx = empty_ctx();
        ctx.visible_panes = vec![p1, p2.clone()];
        let r = handle_command_key(&mut s, &ctx, &mut store, ck('A'));
        assert_eq!(r, vec![Effect::Save, Effect::Render]);
        assert_eq!(s.panes.len(), 2);
        assert_eq!(s.panes[1].as_ref().unwrap().id, 11);
    }

}
