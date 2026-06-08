//! `harpoon-zellij` plugin shim.
//!
//! Thin FFI layer over `harpoon-core`. Converts `zellij_tile` types
//! (PaneInfo, TabInfo, Key) into the host-agnostic projections in
//! `harpoon-core` at the FFI boundary, delegates dispatch + render layout
//! decisions to `harpoon-core`, and translates the resulting effects /
//! descriptors back into `zellij_tile` API calls.
//!
//! See `openspec/changes/add-filter-and-jump-modes/design.md` for the full
//! design rationale.

pub(crate) mod persistence;

use std::collections::BTreeMap;

use persistence::Persistence;
use zellij_tile::prelude::*;

use harpoon_core::{
    build_header, build_hint_line, build_row_entries, build_rows, compute_layout_budget, dispatch,
    filtered_indices, focused_idx as core_focused_idx, reanchor_selected_to_focus,
    resolve_restore_round, BookmarkStore, Config, DispatchContext, DispatchState, Effect,
    HighlightKind, InputKey, MatcherImpl, ModifierSet, Pane, RenderRow, VisiblePane,
};

/// Phase 0.1 verified: y=0 IS visible in zellij 0.44.1 floating plugin
/// panes. Not the older "y=0 overlaps with frame" assumption.
const HEADER_BASE_Y: u16 = 0;

// ── Plugin state ───────────────────────────────────────────────────────────────

#[derive(Default)]
struct State {
    /// Pure-logic state mutated by harpoon-core dispatch handlers.
    dispatch_state: DispatchState,
    /// Bookmark store. Owned here so dispatch can mutate via `&mut`.
    store: BookmarkStore,
    /// Disk I/O wrapper.
    persistence: Persistence,
    /// Plugin configuration parsed from `load`'s BTreeMap.
    config: Config,
    /// Static-dispatch matcher selected at load time.
    matcher: MatcherImpl,

    /// Current pane manifest (cached from last `Event::PaneUpdate`).
    pane_manifest: Option<PaneManifest>,
    /// Current tab info (cached from last `Event::TabUpdate`).
    tab_info: Option<Vec<TabInfo>>,
    /// Session name (captured from `Event::SessionUpdate`).
    session_name: Option<String>,

    /// Last computed filtered_indices view (rebuilt per `update()` for
    /// filter mode; empty otherwise). Used by render and by handlers that
    /// need to resolve `selected → panes_idx`.
    last_filtered_indices: Vec<usize>,
}

register_plugin!(State);

// ── ZellijPlugin trait impl ────────────────────────────────────────────────────

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Parse config; init matcher from it. Mode init at default_mode.
        self.config = Config::parse_from_btree(&configuration);
        self.dispatch_state.default_mode = self.config.default_mode;
        self.dispatch_state.mode = self.config.default_mode;
        self.matcher = MatcherImpl::from_config(&self.config);

        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[
            EventType::Key,
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::RunCommandResult,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::TabUpdate(tab_info) => {
                self.tab_info = Some(tab_info);
                self.update_panes();
                should_render = true;
            }
            Event::PaneUpdate(pane_manifest) => {
                self.pane_manifest = Some(pane_manifest);
                self.update_panes();
                should_render = true;
            }
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                let plugin_ids = get_plugin_ids();
                rename_plugin_pane(plugin_ids.plugin_id, "harpoon");
            }
            Event::SessionUpdate(session_infos, _) => {
                if self.session_name.is_none() {
                    if let Some(current) = session_infos.iter().find(|s| s.is_current_session) {
                        self.session_name = Some(current.name.clone());
                        self.persistence.load_from_disk(&self.session_name);
                    }
                }
            }
            Event::RunCommandResult(_exit_code, stdout, _stderr, context) => {
                if context.get("source").map(|s| s.as_str()) == Some("load") {
                    let content = String::from_utf8_lossy(&stdout);
                    if let Err(e) = self.persistence.on_load_command(&mut self.store, &content) {
                        eprintln!("{e}");
                    } else {
                        // After bookmarks load, attempt a restore round in case
                        // panes are already visible.
                        self.update_panes();
                        should_render = true;
                    }
                }
            }
            Event::Key(key) => {
                let input = key_event_to_input(&key);
                let ctx = self.build_dispatch_context();
                let effects = dispatch(
                    &mut self.dispatch_state,
                    &ctx,
                    &mut self.store,
                    input,
                );
                // Stash filtered_indices for render path.
                self.last_filtered_indices = ctx.filtered_indices;
                self.apply_effects(&effects, &mut should_render);
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let rows = rows as u16;
        let cols_u = cols;

        let budget = compute_layout_budget(rows);

        // Build the row source: live panes + placeholders.
        let entries = build_row_entries(&self.dispatch_state, &self.store);

        // Build header.
        let visible_count = entries
            .iter()
            .filter(|e| matches!(e, harpoon_core::RowEntry::Live(_)))
            .count();
        let filter_match_count = self.last_filtered_indices.len();
        let header = build_header(
            &self.dispatch_state,
            visible_count,
            filter_match_count,
            cols_u,
            budget.max_header_height,
        );

        // Render header lines starting at HEADER_BASE_Y.
        let mut y = HEADER_BASE_Y;
        for line in &header.lines {
            let mut t = Text::new(&line.text);
            if let Some(range) = &line.badge_range {
                t = t.color_range(2, range.clone());
            }
            print_text_with_coordinates(t, 0, y as usize, None, None);
            y += 1;
        }

        // Build pane rows.
        let row_descs: Vec<RenderRow> = build_rows(
            &self.dispatch_state,
            &entries,
            &mut self.matcher,
            &self.last_filtered_indices,
            self.config.show_slots,
            budget.max_pane_rows,
        );

        // Render rows.
        for (i, row) in row_descs.iter().enumerate() {
            let row_y = y as usize + i;
            let mut t = Text::new(&row.text);
            match row.highlight_kind {
                HighlightKind::None => {}
                HighlightKind::FuzzyChars => {
                    t = t.color_indices(1, row.highlight_indices.clone());
                }
                HighlightKind::SubstringRange { start, end } => {
                    t = t.color_range(1, start..end);
                }
            }
            if row.is_selected {
                t = t.selected();
            }
            print_text_with_coordinates(t, 0, row_y, None, None);
        }

        // Render hint line at bottom (if budget allows).
        if budget.hint_visible {
            let hint = build_hint_line(self.dispatch_state.mode, cols_u);
            let hint_y = (rows as usize).saturating_sub(1);
            print_text_with_coordinates(Text::new(&hint), 0, hint_y, None, None);
        }
    }
}

// ── State helpers ──────────────────────────────────────────────────────────────

impl State {
    /// Build the per-event `DispatchContext` from cached pane_manifest +
    /// tab_info. Sorts visible_panes by (tab.position ASC, PaneInfo.id ASC)
    /// for deterministic `A` add-all order. Computes filtered_indices using
    /// the active matcher.
    fn build_dispatch_context(&mut self) -> DispatchContext {
        let visible_panes = self.collect_visible_panes_sorted();
        let focused_pane = self.collect_focused_pane();

        // Update focused_pane_id on dispatch_state so reanchor helpers see it.
        // Sticky semantics: only overwrite when there IS a real terminal
        // focus (per the existing fork's behavior, avoid clobbering when
        // harpoon itself is focused).
        if let Some(p) = &focused_pane {
            self.dispatch_state.focused_pane_id = Some(p.id);
        }

        let f_indices = filtered_indices(&self.dispatch_state, &mut self.matcher);

        DispatchContext {
            focused_pane,
            visible_panes,
            filtered_indices: f_indices,
        }
    }

    /// Collect all visible non-plugin panes from the manifest, sorted by
    /// (tab.position ASC, PaneInfo.id ASC).
    fn collect_visible_panes_sorted(&self) -> Vec<Pane> {
        let mut out: Vec<Pane> = Vec::new();
        let Some(manifest) = self.pane_manifest.as_ref() else {
            return out;
        };
        let Some(tabs) = self.tab_info.as_ref() else {
            return out;
        };
        for tab in tabs {
            let tab_pos_usize = tab.position;
            let Some(pane_infos) = manifest.panes.get(&tab_pos_usize) else {
                continue;
            };
            let mut by_id: Vec<&PaneInfo> = pane_infos.iter().filter(|p| !p.is_plugin).collect();
            by_id.sort_by_key(|p| p.id);
            for pi in by_id {
                out.push(pane_info_to_pane(pi, &tab.name, tab.position as u32));
            }
        }
        // Sort overall by (tab_position ASC, id ASC) — the per-tab loop
        // above iterates tabs in their natural order; deterministic ordering
        // is a secondary sort here.
        out.sort_by(|a, b| {
            a.tab_position
                .cmp(&b.tab_position)
                .then_with(|| a.id.cmp(&b.id))
        });
        out
    }

    /// Find the user's currently-focused terminal pane (the one they came
    /// from when opening harpoon).
    fn collect_focused_pane(&self) -> Option<Pane> {
        let manifest = self.pane_manifest.as_ref()?;
        let tabs = self.tab_info.as_ref()?;
        let active_tab = tabs.iter().find(|t| t.active)?;
        let pane_infos = manifest.panes.get(&active_tab.position)?;
        // Match the existing fork's tie-break: highest pane id among focused
        // non-plugin panes.
        let focused = pane_infos
            .iter()
            .filter(|p| p.is_focused && !p.is_plugin)
            .max_by_key(|p| p.id)?;
        Some(pane_info_to_pane(
            focused,
            &active_tab.name,
            active_tab.position as u32,
        ))
    }

    /// Reconcile state.panes with the latest manifest: drop disappeared
    /// panes, run restore resolution, re-anchor selected.
    fn update_panes(&mut self) {
        let Some(manifest) = self.pane_manifest.clone() else {
            return;
        };
        let Some(_tabs) = self.tab_info.clone() else {
            return;
        };

        // Build set of currently-valid pane ids.
        let mut valid_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for (_, pane_infos) in &manifest.panes {
            for pi in pane_infos {
                if !pi.is_plugin {
                    valid_ids.insert(pi.id);
                }
            }
        }

        // Drop panes whose id is no longer valid. Pre-freeze (sparse) keeps
        // None placeholders untouched; post-freeze (dense) compacts the
        // disappearance.
        if !self.store.frozen {
            // Pre-freeze: turn invalid Some entries back into None.
            for opt in self.dispatch_state.panes.iter_mut() {
                if let Some(p) = opt.as_ref() {
                    if !valid_ids.contains(&p.id) {
                        // Remove map entry; bookmark stays in store.bookmarks.
                        self.store.pane_id_to_bookmark_idx.remove(&p.id);
                        *opt = None;
                    }
                }
            }
        } else {
            // Post-freeze: drop invalid entries entirely.
            let mut new_panes: Vec<Option<Pane>> = Vec::new();
            let mut removed_ids: Vec<u32> = Vec::new();
            for opt in self.dispatch_state.panes.drain(..) {
                if let Some(p) = opt {
                    if valid_ids.contains(&p.id) {
                        new_panes.push(Some(p));
                    } else {
                        removed_ids.push(p.id);
                    }
                }
                // We intentionally drop trailing Nones too post-freeze.
            }
            self.dispatch_state.panes = new_panes;
            // Remove map entries + bookmarks for removed panes; reindex.
            for id in removed_ids {
                if let Some(bk_idx) = self.store.pane_id_to_bookmark_idx.remove(&id) {
                    if bk_idx < self.store.bookmarks.len() {
                        self.store.bookmarks.remove(bk_idx);
                        // Shift map values > bk_idx down by 1.
                        for v in self.store.pane_id_to_bookmark_idx.values_mut() {
                            if *v > bk_idx {
                                *v -= 1;
                            }
                        }
                    }
                }
            }
            // Reassign bookmark indices to new dense positions.
            let mut id_to_new_idx: BTreeMap<u32, u16> = BTreeMap::new();
            for (i, opt) in self.dispatch_state.panes.iter().enumerate() {
                if let Some(p) = opt {
                    id_to_new_idx.insert(p.id, i as u16);
                }
            }
            for (pane_id, &bk_idx) in &self.store.pane_id_to_bookmark_idx {
                if let Some(new_idx) = id_to_new_idx.get(pane_id) {
                    if let Some(b) = self.store.bookmarks.get_mut(bk_idx) {
                        b.index = Some(*new_idx);
                    }
                }
            }
        }

        // Run restore resolution against currently visible panes.
        let visible: Vec<VisiblePane> = self
            .collect_visible_panes_sorted()
            .into_iter()
            .map(|p| VisiblePane {
                id: p.id,
                tab_name: p.tab_name,
                pane_title: p.pane_title,
                tab_position: p.tab_position,
            })
            .collect();
        resolve_restore_round(&mut self.store, &mut self.dispatch_state.panes, &visible);

        // Update focused_pane_id (sticky).
        if let Some(p) = self.collect_focused_pane() {
            self.dispatch_state.focused_pane_id = Some(p.id);
        }

        // Re-anchor selected to focused pane (gated to Command || Filter+empty).
        let f_idx = core_focused_idx(
            &self.dispatch_state.panes,
            self.dispatch_state.focused_pane_id,
        );
        reanchor_selected_to_focus(&mut self.dispatch_state, f_idx);

        // Save if changed.
        self.persistence
            .save_if_changed(&self.store, &self.session_name);
    }

    /// Apply a `Vec<Effect>` from the dispatch core. `Effect::Close` triggers
    /// `hide_self()` + close-helper reset; `Effect::FocusPane(id)` triggers
    /// `focus_terminal_pane`; `Effect::Save` saves persistence; `Effect::Render`
    /// flips `should_render = true`; `Effect::Noop` is ignored.
    ///
    /// Order is significant for `Close ↔ FocusPane`: handlers MUST emit them
    /// as `[Close, FocusPane]` so `hide_self()` runs before
    /// `focus_terminal_pane()`.
    fn apply_effects(&mut self, effects: &[Effect], should_render: &mut bool) {
        for effect in effects {
            match effect {
                Effect::Render => {
                    *should_render = true;
                }
                Effect::Close => {
                    self.close_helper();
                    *should_render = true;
                }
                Effect::FocusPane(id) => {
                    // TODO: This has a bug on macOS with hidden panes.
                    focus_terminal_pane(*id, true);
                    // Full-screen the pane the user jumped to. Target by id
                    // rather than relying on focus having landed, so there is
                    // no race with the focus_terminal_pane call above. This is
                    // a toggle in zellij's API, but a freshly jumped-to pane is
                    // never already fullscreen, so it always results in
                    // fullscreen here.
                    toggle_pane_id_fullscreen(PaneId::Terminal(*id));
                }
                Effect::Save => {
                    self.persistence
                        .save_if_changed(&self.store, &self.session_name);
                }
                Effect::Noop => {}
            }
        }
    }

    /// Single canonical close path: `hide_self()`, reset mode to default,
    /// clear query, re-anchor selected so the next open lands on a valid
    /// index.
    fn close_helper(&mut self) {
        hide_self();
        self.dispatch_state.mode = self.dispatch_state.default_mode;
        self.dispatch_state.query.clear();
        let f_idx = core_focused_idx(
            &self.dispatch_state.panes,
            self.dispatch_state.focused_pane_id,
        );
        reanchor_selected_to_focus(&mut self.dispatch_state, f_idx);
    }
}

// ── FFI conversion helpers ──────────────────────────────────────────────────────

/// Convert a `zellij_tile::PaneInfo` + tab metadata into the host-agnostic
/// `harpoon_core::Pane` projection.
fn pane_info_to_pane(info: &PaneInfo, tab_name: &str, tab_position: u32) -> Pane {
    Pane {
        id: info.id,
        tab_name: tab_name.to_owned(),
        pane_title: info.title.clone(),
        tab_position,
    }
}

/// Convert a `zellij_tile::Key` into `InputKey` with FFI normalization.
///
/// **Normalization** (per Phase 0.3 verification + `design.md` "Decision:
/// Modifier-gated key consumption with FFI normalization"): the host emits
/// shifted ASCII letters as BOTH uppercase char AND `KeyModifier::Shift` set.
/// Drop the Shift bit on those so handlers see the canonical
/// `InputKey::Char('K', ModifierSet::PLAIN)` form.
fn key_event_to_input(key: &KeyWithModifier) -> InputKey {
    let mut modifiers = ModifierSet {
        ctrl: key.has_modifiers(&[KeyModifier::Ctrl]),
        alt: key.has_modifiers(&[KeyModifier::Alt]),
        shift: key.has_modifiers(&[KeyModifier::Shift]),
        super_: key.has_modifiers(&[KeyModifier::Super]),
    };

    match &key.bare_key {
        BareKey::Char(c) => {
            // FFI normalization: drop Shift on ASCII alphabetic uppercase.
            if c.is_ascii_uppercase() && modifiers.shift {
                modifiers.shift = false;
            }
            InputKey::Char(*c, modifiers)
        }
        BareKey::Backspace => InputKey::Backspace,
        BareKey::Esc => InputKey::Esc,
        BareKey::Enter => InputKey::Enter,
        BareKey::Up => InputKey::ArrowUp,
        BareKey::Down => InputKey::ArrowDown,
        _ => InputKey::Other,
    }
}
