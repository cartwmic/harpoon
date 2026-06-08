//! Host-agnostic projection of a tracked pane.
//!
//! The plugin shim converts `zellij_tile::PaneInfo` + `zellij_tile::TabInfo`
//! into this projection at the FFI boundary so that `harpoon-core` code never
//! depends on `zellij-tile` types and can be unit-tested natively.

use std::fmt;

/// A tracked pane. Mirrors what we need from `zellij_tile::PaneInfo`+`TabInfo`
/// but adds nothing host-specific.
///
/// Identity for in-session tracking is `id` (zellij's monotonically-assigned
/// per-session pane id). Identity for cross-reload bookmark resolution is
/// `(tab_name, pane_title)` per the existing `harpoon-plugin/src/persistence.rs`
/// convention.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pane {
    /// Zellij's `PaneInfo.id` — unique per session, monotonically assigned.
    pub id: u32,
    /// Owning tab's name (used for cross-reload identity).
    pub tab_name: String,
    /// Pane title (used for cross-reload identity).
    pub pane_title: String,
    /// Owning tab's `position` (display-order index from zellij).
    pub tab_position: u32,
}

impl fmt::Display for Pane {
    /// Renders as `"<tab_name> | <pane_title>"`. This is the haystack the
    /// matcher consumes (slot prefix is added by the render layer separately
    /// and is excluded from match input per `specs/filter-mode/spec.md`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} | {}", self.tab_name, self.pane_title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: u32, tab: &str, title: &str) -> Pane {
        Pane {
            id,
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            tab_position: 0,
        }
    }

    #[test]
    fn display_format_matches_matcher_haystack() {
        let pane = p(1, "work", "nvim main.rs");
        assert_eq!(pane.to_string(), "work | nvim main.rs");
    }

    #[test]
    fn equal_panes_are_eq() {
        assert_eq!(p(1, "work", "nvim"), p(1, "work", "nvim"));
    }

    #[test]
    fn different_id_not_eq() {
        assert_ne!(p(1, "work", "nvim"), p(2, "work", "nvim"));
    }

    #[test]
    fn duplicate_identity_distinguishable_by_id() {
        // Two panes with identical (tab_name, pane_title) but different ids
        // are NOT equal by the derived PartialEq — the in-memory identity
        // includes the id. Cross-reload identity (which collapses on duplicate
        // titles) is handled by Persistence, not by PartialEq.
        let a = p(1, "work", "nvim");
        let b = p(2, "work", "nvim");
        assert_ne!(a, b);
        assert_eq!(a.tab_name, b.tab_name);
        assert_eq!(a.pane_title, b.pane_title);
    }

    #[test]
    fn display_with_multibyte_chars() {
        let pane = p(1, "📦 build", "tail log");
        assert_eq!(pane.to_string(), "📦 build | tail log");
    }
}
