//! Interaction modes: Command (default), Filter (type-to-filter), Jump (slot keys).
//!
//! See `specs/mode-state-machine/spec.md` for the full state machine.

use std::fmt;

/// The three mutually-exclusive interaction modes.
///
/// At any moment the plugin is in exactly one mode. Transitions between modes
/// are governed by `specs/mode-state-machine/spec.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Today's bare-key behavior: `a`/`A`/`d`/`j`/`k`/`K`/`J`/`l`/`Enter`/`c` etc.
    Command,
    /// Type-to-filter mode. Printable characters append to a query buffer.
    Filter,
    /// Read-only slot-jump mode. Pressing a slot key (`1-9` or `a-z`) jumps to
    /// that pane and closes the plugin. All other keys (except `Esc`) are
    /// ignored.
    Jump,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Command
    }
}

impl Mode {
    /// Parse a config string (case-insensitive). Unknown values map to `None`,
    /// letting the caller apply its fallback rule (typically `Command`).
    pub fn parse_config_value(s: &str) -> Option<Mode> {
        if s.eq_ignore_ascii_case("command") {
            Some(Mode::Command)
        } else if s.eq_ignore_ascii_case("filter") {
            Some(Mode::Filter)
        } else if s.eq_ignore_ascii_case("jump") {
            Some(Mode::Jump)
        } else {
            None
        }
    }

    /// Single-letter badge for header rendering: `[N]` / `[F]` / `[J]`.
    ///
    /// Per `specs/mode-state-machine/spec.md` the badge text is the primary
    /// mode discriminator (zellij-tile's color levels are theme-driven and
    /// uniform across modes at a fixed level).
    pub fn badge_letter(self) -> char {
        match self {
            Mode::Command => 'N',
            Mode::Filter => 'F',
            Mode::Jump => 'J',
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Mode::Command => "command",
            Mode::Filter => "filter",
            Mode::Jump => "jump",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_case_insensitive() {
        assert_eq!(Mode::parse_config_value("command"), Some(Mode::Command));
        assert_eq!(Mode::parse_config_value("COMMAND"), Some(Mode::Command));
        assert_eq!(Mode::parse_config_value("Command"), Some(Mode::Command));
    }

    #[test]
    fn parse_filter_case_insensitive() {
        assert_eq!(Mode::parse_config_value("filter"), Some(Mode::Filter));
        assert_eq!(Mode::parse_config_value("FILTER"), Some(Mode::Filter));
    }

    #[test]
    fn parse_jump_case_insensitive() {
        assert_eq!(Mode::parse_config_value("jump"), Some(Mode::Jump));
        assert_eq!(Mode::parse_config_value("Jump"), Some(Mode::Jump));
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert_eq!(Mode::parse_config_value("wibble"), None);
        assert_eq!(Mode::parse_config_value(""), None);
        assert_eq!(Mode::parse_config_value("commando"), None);
    }

    #[test]
    fn default_is_command() {
        assert_eq!(Mode::default(), Mode::Command);
    }

    #[test]
    fn badge_letters() {
        assert_eq!(Mode::Command.badge_letter(), 'N');
        assert_eq!(Mode::Filter.badge_letter(), 'F');
        assert_eq!(Mode::Jump.badge_letter(), 'J');
    }

    #[test]
    fn display_lowercase() {
        assert_eq!(Mode::Command.to_string(), "command");
        assert_eq!(Mode::Filter.to_string(), "filter");
        assert_eq!(Mode::Jump.to_string(), "jump");
    }
}
