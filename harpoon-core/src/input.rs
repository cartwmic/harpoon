//! Host-agnostic input-key abstraction.
//!
//! The plugin shim converts `zellij_tile::Key { bare_key, key_modifiers }`
//! into `InputKey` at the FFI boundary, applying the ASCII-letter Shift
//! normalization rule (Shift+letter → uppercase + empty ModifierSet) before
//! handlers see the input. See `design.md` "Modifier-gated key consumption
//! with FFI normalization".

/// Modifier set for `InputKey::Char`.
///
/// Constructed at the FFI boundary; handlers gate behavior on `is_plain()`
/// (modifier set empty) for letter/digit keys, with one carve-out: `c` in
/// command mode accepts any modifier set so today's accidental `Ctrl+c` close
/// keeps working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ModifierSet {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_: bool,
}

impl ModifierSet {
    /// Empty modifier set. Construct via `ModifierSet::default()` in handler tests.
    pub const PLAIN: ModifierSet = ModifierSet {
        ctrl: false,
        alt: false,
        shift: false,
        super_: false,
    };

    /// True iff no modifier is set.
    pub fn is_plain(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.super_
    }

    /// True iff at most Shift is set (no Ctrl/Alt/Super).
    ///
    /// Kept for spec readability; FFI-level normalization collapses Shift on
    /// ASCII letters before handlers see them, so for letter/digit gating
    /// `is_plain()` is the canonical check post-normalization.
    pub fn is_plain_or_shift(&self) -> bool {
        !self.ctrl && !self.alt && !self.super_
    }
}

/// Host-agnostic key event. Constructed by the FFI shim from
/// `zellij_tile::Key`; handlers match on this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKey {
    /// A printable character with modifier set. ASCII-letter inputs have been
    /// FFI-normalized: `Char('K', ModifierSet::PLAIN)` rather than
    /// `Char('k', shift=true)`. Symbol keys (`/`, `#`, etc.) may carry
    /// non-empty modifier sets depending on keyboard layout.
    Char(char, ModifierSet),
    Backspace,
    Esc,
    Enter,
    ArrowUp,
    ArrowDown,
    /// Catch-all for keys harpoon doesn't bind (function keys, Tab, etc.).
    /// Handlers always return `vec![Effect::Noop]` for `Other`.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_set_default_is_plain() {
        assert!(ModifierSet::default().is_plain());
    }

    #[test]
    fn plain_const_matches_default() {
        assert_eq!(ModifierSet::PLAIN, ModifierSet::default());
    }

    #[test]
    fn ctrl_is_not_plain() {
        let m = ModifierSet {
            ctrl: true,
            ..Default::default()
        };
        assert!(!m.is_plain());
        assert!(!m.is_plain_or_shift());
    }

    #[test]
    fn shift_alone_is_not_plain_but_is_plain_or_shift() {
        let m = ModifierSet {
            shift: true,
            ..Default::default()
        };
        assert!(!m.is_plain());
        assert!(m.is_plain_or_shift());
    }

    #[test]
    fn alt_is_not_plain_or_shift() {
        let m = ModifierSet {
            alt: true,
            ..Default::default()
        };
        assert!(!m.is_plain_or_shift());
    }

    #[test]
    fn input_key_char_equality_includes_modifiers() {
        let a = InputKey::Char('a', ModifierSet::PLAIN);
        let b = InputKey::Char(
            'a',
            ModifierSet {
                ctrl: true,
                ..Default::default()
            },
        );
        assert_ne!(a, b);
    }

    #[test]
    fn input_key_other_distinct() {
        assert_ne!(InputKey::Other, InputKey::Esc);
        assert_ne!(InputKey::Other, InputKey::Backspace);
    }
}
