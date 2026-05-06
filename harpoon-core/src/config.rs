//! Plugin config parsed from `ZellijPlugin::load`'s `BTreeMap<String, String>`.
//!
//! See `specs/plugin-config/spec.md`. All keys are tolerant: missing keys use
//! defaults; unknown values use defaults (callers may log to stderr).

use std::collections::BTreeMap;

use crate::mode::Mode;

/// Which fuzzy-match algorithm to use.
///
/// Only two backends; the open-set extensibility of `dyn Trait` is unused.
/// Extending requires a new variant + a corresponding `MatcherImpl` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatcherKind {
    /// nucleo-matcher-backed fuzzy match with score + char indices.
    Fuzzy,
    /// Case-insensitive ASCII-fold substring match (always contiguous indices).
    Substring,
}

impl Default for MatcherKind {
    fn default() -> Self {
        MatcherKind::Fuzzy
    }
}

impl MatcherKind {
    pub fn parse_config_value(s: &str) -> Option<MatcherKind> {
        if s.eq_ignore_ascii_case("fuzzy") {
            Some(MatcherKind::Fuzzy)
        } else if s.eq_ignore_ascii_case("substring") {
            Some(MatcherKind::Substring)
        } else {
            None
        }
    }
}

/// Plugin configuration parsed from `ZellijPlugin::load`.
///
/// All fields default to safe values; any missing or unknown config key falls
/// back to the default. Reading is one-shot at load time; this struct is
/// stable for the lifetime of the plugin instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Initial mode and the mode reset target on close.
    pub default_mode: Mode,
    /// Fuzzy or substring matching.
    pub matcher: MatcherKind,
    /// Whether to render slot-prefix characters (1-9, a-z) in command/jump.
    pub show_slots: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_mode: Mode::default(),
            matcher: MatcherKind::default(),
            show_slots: true,
        }
    }
}

/// Parse case-insensitive boolean strings. Accepts `true`/`false`,
/// `yes`/`no`, `on`/`off`, `1`/`0`. Unknown values return `None` so the caller
/// can apply its default.
fn parse_bool(s: &str) -> Option<bool> {
    if s.eq_ignore_ascii_case("true")
        || s.eq_ignore_ascii_case("yes")
        || s.eq_ignore_ascii_case("on")
        || s == "1"
    {
        Some(true)
    } else if s.eq_ignore_ascii_case("false")
        || s.eq_ignore_ascii_case("no")
        || s.eq_ignore_ascii_case("off")
        || s == "0"
    {
        Some(false)
    } else {
        None
    }
}

impl Config {
    /// Parse from a zellij plugin config map. Keys are case-sensitive
    /// (zellij convention is lowercase-with-underscores); values are
    /// case-insensitive per the requirement.
    pub fn parse_from_btree(map: &BTreeMap<String, String>) -> Config {
        let mut cfg = Config::default();
        if let Some(v) = map.get("default_mode") {
            if let Some(m) = Mode::parse_config_value(v) {
                cfg.default_mode = m;
            }
        }
        if let Some(v) = map.get("matcher") {
            if let Some(m) = MatcherKind::parse_config_value(v) {
                cfg.matcher = m;
            }
        }
        if let Some(v) = map.get("show_slots") {
            if let Some(b) = parse_bool(v) {
                cfg.show_slots = b;
            }
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn defaults_when_empty() {
        let c = Config::parse_from_btree(&map(&[]));
        assert_eq!(c.default_mode, Mode::Command);
        assert_eq!(c.matcher, MatcherKind::Fuzzy);
        assert!(c.show_slots);
    }

    #[test]
    fn default_mode_filter() {
        let c = Config::parse_from_btree(&map(&[("default_mode", "filter")]));
        assert_eq!(c.default_mode, Mode::Filter);
    }

    #[test]
    fn default_mode_case_insensitive() {
        let c = Config::parse_from_btree(&map(&[("default_mode", "JUMP")]));
        assert_eq!(c.default_mode, Mode::Jump);
    }

    #[test]
    fn default_mode_unknown_falls_back() {
        let c = Config::parse_from_btree(&map(&[("default_mode", "wibble")]));
        assert_eq!(c.default_mode, Mode::Command);
    }

    #[test]
    fn matcher_substring() {
        let c = Config::parse_from_btree(&map(&[("matcher", "substring")]));
        assert_eq!(c.matcher, MatcherKind::Substring);
    }

    #[test]
    fn matcher_unknown_falls_back() {
        let c = Config::parse_from_btree(&map(&[("matcher", "regex")]));
        assert_eq!(c.matcher, MatcherKind::Fuzzy);
    }

    #[test]
    fn show_slots_off_via_string() {
        let c = Config::parse_from_btree(&map(&[("show_slots", "false")]));
        assert!(!c.show_slots);
    }

    #[test]
    fn show_slots_off_via_no() {
        let c = Config::parse_from_btree(&map(&[("show_slots", "no")]));
        assert!(!c.show_slots);
    }

    #[test]
    fn show_slots_off_case_insensitive() {
        let c = Config::parse_from_btree(&map(&[("show_slots", "FALSE")]));
        assert!(!c.show_slots);
    }

    #[test]
    fn show_slots_unknown_falls_back_to_true() {
        // Default for show_slots is `true`; unknown values keep the default.
        let c = Config::parse_from_btree(&map(&[("show_slots", "maybe")]));
        assert!(c.show_slots);
    }

    #[test]
    fn all_three_keys_at_once() {
        let c = Config::parse_from_btree(&map(&[
            ("default_mode", "filter"),
            ("matcher", "substring"),
            ("show_slots", "false"),
        ]));
        assert_eq!(c.default_mode, Mode::Filter);
        assert_eq!(c.matcher, MatcherKind::Substring);
        assert!(!c.show_slots);
    }
}
