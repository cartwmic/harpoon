//! `harpoon-core` — pure-logic core for the harpoon-zellij plugin.
//!
//! This crate is intentionally free of any `zellij-tile` dependency so that
//! the dispatch core, matcher, mode state machine, and render-layout helpers
//! can be unit-tested natively (`cargo test -p harpoon-core` from the
//! workspace root).
//!
//! The plugin shim (`harpoon-plugin`) converts `zellij_tile::PaneInfo` and
//! `zellij_tile::Key` into the host-agnostic projections defined here at the
//! FFI boundary, then delegates all behavior to functions in this crate.
//!
//! See `openspec/changes/add-filter-and-jump-modes/design.md` for the full
//! design rationale.

pub mod config;
pub mod effect;
pub mod input;
pub mod mode;
pub mod pane;

// Re-export the most-used types at the crate root for ergonomics.
pub use config::{Config, MatcherKind};
pub use effect::Effect;
pub use input::{InputKey, ModifierSet};
pub use mode::Mode;
pub use pane::Pane;
