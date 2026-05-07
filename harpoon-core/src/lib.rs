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

pub mod bookmark;
pub mod command;
pub mod config;
pub mod dispatch;
pub mod effect;
pub mod filter;
pub mod freeze;
pub mod input;
pub mod jump;
pub mod matcher;
pub mod mode;
pub mod pane;
pub mod slot;

// Re-export the most-used types at the crate root for ergonomics.
pub use bookmark::{BookmarkStore, PaneBookmark};
pub use command::handle_command_key;
pub use config::{Config, MatcherKind};
pub use dispatch::{
    clamp_selected_to_view, dispatch, focused_idx, reanchor_selected_to_focus, DispatchContext,
    DispatchState,
};
pub use effect::Effect;
pub use filter::{filtered_indices, handle_filter_key};
pub use freeze::freeze_on_user_mutation;
pub use input::{InputKey, ModifierSet};
pub use jump::handle_jump_key;
pub use matcher::{FuzzyMatcher, MatchResult, Matcher, MatcherImpl, SubstringMatcher};
pub use mode::Mode;
pub use pane::Pane;
pub use slot::{slot_char_from_index, slot_index_from_char};
