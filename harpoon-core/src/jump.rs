//! Jump-mode key dispatch.
//!
//! Read-only mode: pressing a slot key (digit `1-9` or letter `a-z`) jumps
//! to that pane and closes the plugin. `Esc` returns to command mode. ALL
//! other keys are ignored — no mutations of `state.panes`, `state.query`,
//! or persistence happen here. This read-only property is what makes jump
//! mode safe as a default.
//!
//! See `specs/jump-mode/spec.md` for the full contract.

use crate::dispatch::{DispatchContext, DispatchState};
use crate::effect::Effect;
use crate::input::InputKey;
use crate::mode::Mode;
use crate::slot::slot_index_from_char;

/// Jump-mode dispatch.
pub fn handle_jump_key(
    state: &mut DispatchState,
    _ctx: &DispatchContext,
    key: InputKey,
) -> Vec<Effect> {
    match key {
        // Esc returns to command mode.
        InputKey::Esc => {
            state.mode = Mode::Command;
            vec![Effect::Render]
        }

        // Slot keys: any digit or letter (modifier-gated to plain post-FFI
        // normalization). Live → close+focus; placeholder → no-op;
        // OOB → no-op.
        InputKey::Char(c, modifiers) => {
            if !modifiers.is_plain() {
                return Vec::new();
            }
            let Some(slot_idx) = slot_index_from_char(c) else {
                return Vec::new();
            };
            match state.panes.get(slot_idx).and_then(|opt| opt.as_ref()) {
                Some(pane) => vec![Effect::Close, Effect::FocusPane(pane.id)],
                None => Vec::new(),
            }
        }

        // All other keys (Backspace, Enter, ArrowUp/Down, Other) — ignored.
        InputKey::Backspace
        | InputKey::Enter
        | InputKey::ArrowUp
        | InputKey::ArrowDown
        | InputKey::Other => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ModifierSet;
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

    fn ctrl() -> ModifierSet {
        ModifierSet {
            ctrl: true,
            ..Default::default()
        }
    }

    fn jump_state(panes: Vec<Option<Pane>>) -> DispatchState {
        let mut s = DispatchState::default();
        s.mode = Mode::Jump;
        s.panes = panes;
        s
    }

    fn empty_ctx() -> DispatchContext {
        DispatchContext::default()
    }

    // ── Esc returns to command ────────────────────────────────────────────

    #[test]
    fn esc_returns_to_command_mode() {
        let mut s = jump_state(vec![]);
        let r = handle_jump_key(&mut s, &empty_ctx(), InputKey::Esc);
        assert_eq!(r, vec![Effect::Render]);
        assert_eq!(s.mode, Mode::Command);
    }

    // ── Digit slot jumps ──────────────────────────────────────────────────

    #[test]
    fn digit_1_on_live_returns_close_focus() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('1'));
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(10)]);
    }

    #[test]
    fn digit_9_on_live_returns_close_focus() {
        let panes: Vec<Option<Pane>> = (0..9).map(|i| Some(p(i + 1, "t", "x"))).collect();
        let mut s = jump_state(panes);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('9'));
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(9)]);
    }

    #[test]
    fn digit_on_placeholder_returns_empty() {
        // panes[1] is None placeholder; pressing `2` no-ops to preserve
        // saved-position contract during partial restore.
        let mut s = jump_state(vec![Some(p(10, "a", "x")), None]);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('2'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn digit_oob_returns_empty() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('5'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── Letter slot jumps (jump mode supports letters; command does not) ─

    #[test]
    fn letter_b_on_live_returns_close_focus() {
        // Slot 'b' = panes[10]
        let panes: Vec<Option<Pane>> = (0..11).map(|i| Some(p(i + 1, "t", "x"))).collect();
        let mut s = jump_state(panes);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('b'));
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(11)]);
    }

    #[test]
    fn letter_z_on_live_returns_close_focus() {
        // Slot 'z' = panes[34]
        let panes: Vec<Option<Pane>> = (0..35).map(|i| Some(p(i + 1, "t", "x"))).collect();
        let mut s = jump_state(panes);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('z'));
        assert_eq!(r, vec![Effect::Close, Effect::FocusPane(35)]);
    }

    #[test]
    fn letter_on_placeholder_returns_empty() {
        // Slot 'a' = panes[9]; we set panes[9] = None.
        let mut panes: Vec<Option<Pane>> = (0..9).map(|i| Some(p(i + 1, "t", "x"))).collect();
        panes.push(None); // panes[9] is placeholder
        let mut s = jump_state(panes);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('a'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn letter_oob_returns_empty() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('a'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── Read-only: nothing else mutates ───────────────────────────────────

    #[test]
    fn backspace_in_jump_mode_ignored() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(&mut s, &empty_ctx(), InputKey::Backspace);
        assert_eq!(r, Vec::<Effect>::new());
        assert_eq!(s.mode, Mode::Jump, "mode unchanged");
    }

    #[test]
    fn enter_in_jump_mode_ignored() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(&mut s, &empty_ctx(), InputKey::Enter);
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn arrow_keys_ignored() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r1 = handle_jump_key(&mut s, &empty_ctx(), InputKey::ArrowUp);
        let r2 = handle_jump_key(&mut s, &empty_ctx(), InputKey::ArrowDown);
        assert_eq!(r1, Vec::<Effect>::new());
        assert_eq!(r2, Vec::<Effect>::new());
    }

    // ── Modifier gating ───────────────────────────────────────────────────

    #[test]
    fn ctrl_digit_returns_empty() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(
            &mut s,
            &empty_ctx(),
            InputKey::Char('1', ctrl()),
        );
        assert_eq!(r, Vec::<Effect>::new(), "modified slot keys are ignored in jump mode");
    }

    #[test]
    fn alt_letter_returns_empty() {
        let panes: Vec<Option<Pane>> = (0..11).map(|i| Some(p(i + 1, "t", "x"))).collect();
        let mut s = jump_state(panes);
        let r = handle_jump_key(
            &mut s,
            &empty_ctx(),
            InputKey::Char(
                'b',
                ModifierSet {
                    alt: true,
                    ..Default::default()
                },
            ),
        );
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── Non-slot characters ───────────────────────────────────────────────

    #[test]
    fn digit_zero_returns_empty() {
        // '0' is not a valid slot character.
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('0'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn uppercase_letter_returns_empty() {
        let panes: Vec<Option<Pane>> = (0..11).map(|i| Some(p(i + 1, "t", "x"))).collect();
        let mut s = jump_state(panes);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('B'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    #[test]
    fn symbol_returns_empty() {
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let r = handle_jump_key(&mut s, &empty_ctx(), ck('/'));
        assert_eq!(r, Vec::<Effect>::new());
    }

    // ── No state mutation on slot jump ────────────────────────────────────

    #[test]
    fn slot_jump_does_not_mutate_state() {
        // Verifies the read-only property: a slot jump emits effects but
        // doesn't mutate state.panes, state.query, or state.mode (the shim
        // resets mode via close_helper, not the handler).
        let mut s = jump_state(vec![Some(p(10, "a", "x"))]);
        let panes_before = s.panes.clone();
        let mode_before = s.mode;
        let _ = handle_jump_key(&mut s, &empty_ctx(), ck('1'));
        assert_eq!(s.panes, panes_before);
        assert_eq!(s.mode, mode_before, "handler does not change mode (shim does on Close)");
    }
}
