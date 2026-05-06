//! Side-effect descriptors emitted by pure dispatch handlers.
//!
//! `dispatch(state, ctx, key) -> Vec<Effect>` is the pure-logic boundary.
//! The plugin shim translates each `Effect` into a zellij FFI call, after
//! the handler has finished mutating `DispatchState` / `Persistence::bookmarks`.
//!
//! See `design.md` "Decision: Pure dispatch core" and the `Close↔FocusPane`
//! ordering rule.

/// Side effect to be applied by the plugin shim.
///
/// **Ordering rule**: Effects are applied in the order they appear in the
/// returned `Vec<Effect>`. Order is **observably significant only for the
/// `Close ↔ FocusPane` pair**: handlers that focus-and-close MUST emit
/// `[Effect::Close, Effect::FocusPane(id)]` so `hide_self()` runs before
/// `focus_terminal_pane(id, true)`. All other effects are commutative.
///
/// Convention for handler returns: `[Save?, Render?, Close?, FocusPane?, ...]`
/// (mutations → visibility → transitions).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Mark the plugin for re-render. The shim sets `should_render = true`
    /// after the handler returns (idempotent across multiple `Render` effects).
    Render,
    /// Hide the plugin. Shim calls `hide_self()` and runs the close-helper
    /// (resets mode to `default_mode`, clears query, re-anchors `selected`).
    Close,
    /// Focus the terminal pane with this id. Shim calls
    /// `focus_terminal_pane(id, true)`. MUST appear after any `Effect::Close`
    /// in the same Vec to preserve today's `hide_self → focus_terminal_pane`
    /// ordering (matches the carried-forward macOS focus bug semantics).
    FocusPane(u32),
    /// Persist `bookmarks` to disk via `save_if_changed()`. Shim no-ops if the
    /// canonical shape hasn't changed.
    Save,
    /// Explicit no-op. Used by handler arms that consciously declined to
    /// mutate (e.g. modifier-gated rejects); distinguishes "no observable
    /// effect by design" from "no effect because we forgot to handle this".
    Noop,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_pane_carries_id() {
        let e = Effect::FocusPane(42);
        assert_eq!(e, Effect::FocusPane(42));
        assert_ne!(e, Effect::FocusPane(43));
    }

    #[test]
    fn distinct_variants_not_equal() {
        assert_ne!(Effect::Render, Effect::Close);
        assert_ne!(Effect::Close, Effect::Save);
        assert_ne!(Effect::Save, Effect::Noop);
    }

    #[test]
    fn vec_ordering_close_before_focus() {
        // Convention check — Close must appear before FocusPane in handler
        // returns. This test documents the contract; actual handler tests
        // assert against this Vec layout directly.
        let effects = vec![Effect::Close, Effect::FocusPane(7)];
        assert_eq!(effects[0], Effect::Close);
        assert_eq!(effects[1], Effect::FocusPane(7));
    }
}
