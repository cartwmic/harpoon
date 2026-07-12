//! Pure helpers backing the plugin's external CLI-pipe interface
//! (`pane-pipe-api` capability): pane-id string parsing and pane→slot reverse
//! lookup. Kept in `harpoon-core` so they are unit-testable natively, free of
//! any `zellij-tile` dependency. The plugin shim (`harpoon-plugin`) calls these
//! from its `pipe()` handler and wires the results to `cli_pipe_output` /
//! `jump_focus_fullscreen`.

use crate::bookmark::BookmarkStore;
use crate::effect::Effect;

/// Parse a zellij pane-id string into a terminal pane id.
///
/// Accepts both forms zellij uses for terminal panes:
/// - `terminal_N` — the form exported to every pane as `$ZELLIJ_PANE_ID`.
/// - bare `N` — the integer form (equivalent to `terminal_N`).
///
/// Returns `None` for anything else: an empty string, a non-numeric tail, a
/// non-terminal pane kind (e.g. `plugin_3`), or a value that does not fit a
/// `u32`. Matching the string form here reconciles `$ZELLIJ_PANE_ID` with
/// harpoon's stored `PaneInfo.id` (`u32`).
///
/// AC: `pane-pipe-api.pane-id-string-parsing`.
pub fn parse_pane_id(raw: &str) -> Option<u32> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let digits = s.strip_prefix("terminal_").unwrap_or(s);
    // Reject any other underscore-tagged kind (e.g. `plugin_3`) and empty tails.
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

/// Reverse lookup: the 1-based harpoon slot currently holding terminal pane
/// `id`, or `None` when the pane has no bookmark or its bookmark is not
/// materialized into a slot (`PaneBookmark.index` is `None`).
///
/// The returned value is `PaneBookmark.index + 1` — the 1-based form of the
/// slot index in `DispatchState::panes` (the position a user hotkeys in jump
/// mode). Pure read: never mutates `store`.
///
/// AC: `pane-pipe-api.slot-for-pane-reverse-lookup`.
pub fn slot_for_pane(store: &BookmarkStore, id: u32) -> Option<u16> {
    let &bi = store.pane_id_to_bookmark_idx.get(&id)?;
    let bookmark = store.bookmarks.get(bi)?;
    bookmark.index.map(|i| i + 1)
}

/// Ground truth about the target pane's tab fullscreen state, queried from
/// the host AFTER the target pane has been focused.
///
/// Constructed by the shim from a synchronous post-focus host query
/// (`get_focused_pane_info` → `get_tab_info().is_fullscreen_active`), never
/// from event-cached `TabInfo`/`PaneInfo` snapshots — caches are `None` on a
/// cold pipe-spawned instance and predictions of focus side effects diverge
/// by layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullscreenGroundTruth {
    /// The tab is fullscreen. Since the target was just focused, the target
    /// IS the fullscreen pane — the jump is already in its goal state.
    Fullscreen,
    /// The tab is provably tiled; an enter-toggle is required (a toggle from
    /// tiled state always ENTERS fullscreen — verified zellij 0.44.3 server
    /// semantics).
    Tiled,
    /// Ground truth could not be established (query failed, pane/tab gone).
    Unknown,
}

/// Post-focus fullscreen effect plan for the jump path (core-owned decision,
/// Constitution I): given ground truth queried AFTER the target pane `id`
/// was focused, emit the ordered effects the shim must apply.
///
/// - `Fullscreen` → `[]`: goal state already holds — the just-focused target
///   IS the fullscreen pane; a toggle would EXIT fullscreen (the historical
///   bug).
/// - `Tiled` → `[Effect::ToggleFullscreenPane(id)]`: the only state needing
///   a toggle; from tiled it can only enter fullscreen, so the wrong
///   direction is structurally impossible.
/// - `Unknown` → `[]`: never toggle from unknown state — zellij has no
///   absolute set-fullscreen, so a blind toggle is wrong half the time
///   (Constitution IV; domain invariant 1).
///
/// AC: `pane-pipe-api.ground-truth-fullscreen-normalization`.
/// AC: `pane-pipe-api.jump-to-pane-by-id`.
pub fn post_focus_fullscreen_plan(truth: FullscreenGroundTruth, id: u32) -> Vec<Effect> {
    match truth {
        FullscreenGroundTruth::Tiled => vec![Effect::ToggleFullscreenPane(id)],
        FullscreenGroundTruth::Fullscreen | FullscreenGroundTruth::Unknown => Vec::new(),
    }
}

/// Synchronously queried host state driving a `toggle` pipe decision
/// (`pane-pipe-api.toggle-state-sync-query-verified`).
///
/// Constructed by the shim from synchronous host queries AT PIPE-HANDLING
/// TIME, never from event caches: probes (2026-07-11, task 1.1/1.2) proved
/// cached `TabUpdate`/`PaneUpdate` FREEZE while the pane is suppressed, and
/// `Event::Visible` is emitted only to tiled plugin panes (a floating plugin
/// never receives it). Constitution IV: never act on unverified host state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleGroundTruth {
    /// `get_pane_info(PaneId::Plugin(own)).is_suppressed`:
    /// - `Some(true)`  — hidden (parked in `suppressed_panes`);
    /// - `Some(false)` — visible (tiled or floating container);
    /// - `None`        — query failed / own pane not yet registered (cold
    ///   spawn window: the pipe can arrive before the pane exists host-side).
    pub own_suppressed: Option<bool>,
    /// The invoking client's focused tab POSITION at pipe time:
    /// `get_focused_pane_info()` (returns the STABLE TAB ID — zellij
    /// `active_tab_ids`) converted via `get_tab_info(id).position`. `None`
    /// when either query fails (cold spawn tolerance).
    pub focused_tab_position: Option<usize>,
}

/// The single host-effect plan a `toggle` pipe message resolves to
/// (`pane-pipe-api.toggle-pipe-invocation` — the four intent branches).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleAction {
    /// Visible → hide. Shim calls `hide_self()` + the close-helper (mode
    /// reset), preserving mode-state-machine Close consolidation semantics.
    Hide,
    /// Hidden (or cold/unknown) with NO reliable relocation target → show
    /// where the pane lives. Shim calls `show_self(true)` — the
    /// position-correct `focus_pane_with_id` host path, safe without any
    /// cached state (covers the cold-spawn branch: the pipe-spawned pane is
    /// parked in the active tab anyway).
    ShowInPlace,
    /// Hidden with a known invoking-tab position → un-suppress FIRST, then
    /// relocate. Shim calls `show_self(true)` and THEN
    /// `break_panes_to_tab_with_index([own], target, true)` — mandatory
    /// ordering: the relocation host call cannot extract suppressed panes
    /// (`extract_pane(_, dont_swap_if_suppressed=true)` returns `None` for
    /// them — the zellij defect-#2 shape). Same-tab invokes collapse into
    /// this arm harmlessly: the break host call skips extraction when the
    /// pane already sits on the target tab and its `go_to_tab(target)` is a
    /// no-op on the already-active tab.
    ShowThenRelocate {
        /// Target tab POSITION (0-based display order), never a stable id.
        target_tab_position: usize,
    },
}

/// Pure branch selection for a `toggle` pipe message (Constitution I: the
/// decision lives in core; the shim only executes the returned action).
///
/// AC: `pane-pipe-api.toggle-pipe-invocation`.
/// AC: `pane-pipe-api.toggle-state-sync-query-verified`.
pub fn toggle_plan(truth: ToggleGroundTruth) -> ToggleAction {
    match (truth.own_suppressed, truth.focused_tab_position) {
        // Visible → hide, regardless of which tab is focused.
        (Some(false), _) => ToggleAction::Hide,
        // Hidden with a verified invoking tab → show, then relocate to it.
        (Some(true), Some(target_tab_position)) => ToggleAction::ShowThenRelocate {
            target_tab_position,
        },
        // Hidden but the focused-tab query failed → show where we live
        // rather than relocating on a guess (Constitution IV).
        (Some(true), None) => ToggleAction::ShowInPlace,
        // Own-pane query failed — cold spawn window or host error. Showing
        // is the only safe default: `show_self` needs no cached state and a
        // pipe-spawned pane parks in the active tab already.
        (None, _) => ToggleAction::ShowInPlace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmark::{BookmarkStore, PaneBookmark};

    // ── parse_pane_id ─ AC pane-pipe-api.pane-id-string-parsing ──────────────

    #[test]
    fn parses_terminal_prefixed_form() {
        assert_eq!(parse_pane_id("terminal_7"), Some(7));
    }

    #[test]
    fn parses_bare_integer_form() {
        assert_eq!(parse_pane_id("7"), Some(7));
    }

    #[test]
    fn parses_zero() {
        assert_eq!(parse_pane_id("terminal_0"), Some(0));
        assert_eq!(parse_pane_id("0"), Some(0));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        // zellij pipe payloads can carry a trailing newline.
        assert_eq!(parse_pane_id(" terminal_12\n"), Some(12));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(parse_pane_id(""), None);
        assert_eq!(parse_pane_id("   "), None);
    }

    #[test]
    fn rejects_plugin_kind() {
        assert_eq!(parse_pane_id("plugin_3"), None);
    }

    #[test]
    fn rejects_non_numeric() {
        assert_eq!(parse_pane_id("abc"), None);
        assert_eq!(parse_pane_id("terminal_"), None);
        assert_eq!(parse_pane_id("terminal_x"), None);
        assert_eq!(parse_pane_id("7a"), None);
    }

    #[test]
    fn rejects_overflow() {
        // u32::MAX + 1 does not fit.
        assert_eq!(parse_pane_id("4294967296"), None);
    }

    // ── slot_for_pane ─ AC pane-pipe-api.slot-for-pane-reverse-lookup ────────

    fn bm(tab: &str, title: &str, index: Option<u16>, id: Option<u32>) -> PaneBookmark {
        PaneBookmark {
            tab_name: tab.to_owned(),
            pane_title: title.to_owned(),
            index,
            id,
        }
    }

    /// Build a store with one bookmark at slot index 2 bound to pane id 7.
    fn store_with_pane7_at_slot2() -> BookmarkStore {
        let mut store = BookmarkStore {
            bookmarks: vec![
                bm("a", "p0", Some(0), Some(5)),
                bm("b", "p1", Some(1), Some(6)),
                bm("c", "p7", Some(2), Some(7)),
            ],
            ..Default::default()
        };
        store.pane_id_to_bookmark_idx.insert(5, 0);
        store.pane_id_to_bookmark_idx.insert(6, 1);
        store.pane_id_to_bookmark_idx.insert(7, 2);
        store
    }

    #[test]
    fn harpooned_pane_returns_one_based_slot() {
        let store = store_with_pane7_at_slot2();
        // slot index 2 → 1-based slot 3
        assert_eq!(slot_for_pane(&store, 7), Some(3));
    }

    #[test]
    fn absent_pane_returns_none() {
        let store = store_with_pane7_at_slot2();
        assert_eq!(slot_for_pane(&store, 999), None);
    }

    #[test]
    fn unmaterialized_bookmark_returns_none() {
        // Bookmark exists and is mapped to a live pane, but has no slot index.
        let mut store = BookmarkStore {
            bookmarks: vec![bm("a", "p", None, Some(42))],
            ..Default::default()
        };
        store.pane_id_to_bookmark_idx.insert(42, 0);
        assert_eq!(slot_for_pane(&store, 42), None);
    }

    #[test]
    fn lookup_does_not_mutate_store() {
        let store = store_with_pane7_at_slot2();
        let before = store.clone();
        let _ = slot_for_pane(&store, 7);
        let _ = slot_for_pane(&store, 999);
        assert_eq!(store.bookmarks, before.bookmarks);
        assert_eq!(store.pane_id_to_bookmark_idx, before.pane_id_to_bookmark_idx);
        assert_eq!(store.frozen, before.frozen);
    }

    // AC: pane-pipe-api.jump-to-pane-by-id — post-focus normalization leaves
    // the target fullscreen in every quadrant reachable at decision time.
    #[test]
    fn fullscreen_tab_plans_no_effects() {
        // Target was just focused; tab fullscreen => target IS the fullscreen
        // pane. A toggle here would exit fullscreen (the historical bug).
        assert!(post_focus_fullscreen_plan(FullscreenGroundTruth::Fullscreen, 7).is_empty());
    }

    #[test]
    fn tiled_tab_plans_enter_toggle_on_target() {
        // AC: pane-pipe-api.jump-to-pane-by-id — the emitted effect carries
        // the target pane id, so the shim cannot toggle the wrong pane.
        assert_eq!(
            post_focus_fullscreen_plan(FullscreenGroundTruth::Tiled, 7),
            vec![Effect::ToggleFullscreenPane(7)]
        );
    }

    // AC: pane-pipe-api.ground-truth-fullscreen-normalization — never toggle
    // from unknown state (no absolute set-fullscreen exists in zellij).
    #[test]
    fn unknown_state_plans_no_effects() {
        assert!(post_focus_fullscreen_plan(FullscreenGroundTruth::Unknown, 7).is_empty());
    }

    // ── toggle_plan ─ AC pane-pipe-api.toggle-pipe-invocation ──────────────

    fn truth(own_suppressed: Option<bool>, focused_tab_position: Option<usize>) -> ToggleGroundTruth {
        ToggleGroundTruth {
            own_suppressed,
            focused_tab_position,
        }
    }

    #[test]
    fn visible_toggles_to_hide() {
        // Branch 1: visible → hide — whatever the focused tab reports.
        assert_eq!(toggle_plan(truth(Some(false), Some(3))), ToggleAction::Hide);
        assert_eq!(toggle_plan(truth(Some(false), None)), ToggleAction::Hide);
    }

    #[test]
    fn hidden_with_target_shows_then_relocates() {
        // Branches 2+3 unified: hidden → show first (un-suppress), then
        // relocate to the verified invoking tab. Same-tab collapses into a
        // harmless no-op relocation host-side.
        assert_eq!(
            toggle_plan(truth(Some(true), Some(1))),
            ToggleAction::ShowThenRelocate {
                target_tab_position: 1
            }
        );
    }

    #[test]
    fn relocation_target_is_a_position_from_sync_query() {
        // AC: pane-pipe-api.toggle-state-sync-query-verified — the target the
        // plan carries is exactly the synchronously queried focused-tab
        // position; core never substitutes cached or stale values.
        for pos in [0usize, 1, 5, 41] {
            assert_eq!(
                toggle_plan(truth(Some(true), Some(pos))),
                ToggleAction::ShowThenRelocate {
                    target_tab_position: pos
                }
            );
        }
    }

    #[test]
    fn hidden_without_target_shows_in_place() {
        // Constitution IV: a failed focused-tab query never relocates on a
        // guess — show where the pane lives instead.
        assert_eq!(toggle_plan(truth(Some(true), None)), ToggleAction::ShowInPlace);
    }

    #[test]
    fn cold_spawn_shows_in_place_without_cached_state() {
        // Branch 4: own-pane query failed (pipe arrived before the pane
        // registered — the cold-start window) → ShowInPlace, which the shim
        // executes via show_self(true), a host call needing NO cached event
        // state. AC scenario: cold spawn shows without cached event state.
        assert_eq!(toggle_plan(truth(None, None)), ToggleAction::ShowInPlace);
        assert_eq!(toggle_plan(truth(None, Some(2))), ToggleAction::ShowInPlace);
    }
}
