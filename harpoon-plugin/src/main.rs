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
    bootstrap_arrival_decision, build_header, build_hint_line, build_row_entries, build_rows,
    compute_layout_budget, decode_bootstrap, disk_load_decision, dispatch, encode_bootstrap,
    filtered_indices, focused_idx as core_focused_idx, guarded_save_decision, manifest_covers_tabs,
    merge_missing, parse_pane_id, post_focus_fullscreen_plan, prune_bookmarks_by_pane_ids,
    reanchor_selected_to_focus, refresh_resolved_identities, resolve_restore_round,
    shrinking_save_allowed, slot_for_pane, store_ready_to_render, toggle_plan, AdoptDecision,
    BookmarkStore, BootstrapPayload, Config, DeferredPruneGuard, DiskLoadDecision, DispatchContext,
    DispatchState, DuplicateToggleGuard, Effect, FullscreenGroundTruth, GuardedSaveDecision,
    HighlightKind, InputKey, MatcherImpl, ModifierSet, Pane, RenderRow, StoreBootState,
    ToggleAction, ToggleGroundTruth, VisiblePane, BOOTSTRAP_PIPE_NAME, BOOTSTRAP_VERSION,
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

    /// Armed by a `toggle` pipe that arrived in the cold-spawn window (own
    /// pane not yet registered host-side — `ToggleAction::ColdShow`).
    /// Resolved on `Event::Timer` (a suppressed pane receives NO
    /// TabUpdate/PaneUpdate events — probe-verified — but timer delivery is
    /// direct) with FRESH sync queries, never cached event content.
    /// AC: pane-pipe-api.toggle-pipe-invocation (cold-spawn scenario).
    pending_cold_show: bool,
    /// Bounded re-arm budget for the cold-show timer (0.2s ticks).
    cold_show_retries: u8,

    /// Verbatim load-time configuration — re-emitted on the respawn branch
    /// so the fresh instance keeps the same pipe-destination identity
    /// (URL + configuration) as the invoking keybind.
    raw_configuration: BTreeMap<String, String>,
    /// Stable tab ID the pane lives on. Recorded ONLY at moments the own
    /// pane is VERIFIABLY the client's focused pane (post-show, pre-hide,
    /// grant-time when self-focused) — then focused tab == own tab by
    /// identity, never a proxy. A focused-tab sample taken while another
    /// pane holds focus (e.g. an ntfy `jump_pane` cold spawn that parks the
    /// pane on one tab and focuses a terminal on another) would poison the
    /// record and re-create the wrong-tab symptom via the warm ShowInPlace
    /// branch (round-2 review finding). Never from event caches
    /// (AC pane-pipe-api.toggle-state-sync-query-verified).
    /// Stale-safe both ways: unknown → Respawn (correct tab, ~100ms); tab
    /// ids are never reused, so a closed parked tab can only compare false
    /// → Respawn.
    parked_tab_id: Option<usize>,
    /// Set on `PermissionRequestResult(Granted)`. Response-decoding sync
    /// queries (`get_pane_info`, `get_focused_pane_info`, `get_tab_info`,
    /// `open_plugin_pane_floating`) PANIC the plugin when permission-denied
    /// (the shim unwraps an empty response — observed 2026-07-12: 'PANIC IN
    /// PLUGIN' after 'GetFocusedPaneInfo denied'), so they are FORBIDDEN
    /// until this is true. Deny-safe calls (show_self/hide_self/set_timeout)
    /// decode no response and stay allowed.
    permissions_granted: bool,
    /// Independently verified grant for the destination-id bootstrap send.
    handoff_permission_granted: bool,
    /// Exactly-one disk load initiation, independent of whether session
    /// name arrived from SessionUpdate or the bootstrap payload.
    disk_load_started: bool,
    /// CLI pipe messages that arrived before the aggregate permission grant.
    /// Running their response-decoding host calls pre-grant panics; dropping
    /// them breaks cold `jump_pane`. Drain exactly once after grant, where
    /// the normal pipe tail also releases each CLI client.
    pending_cli_pipes: Vec<PipeMessage>,

    /// Store-population lifecycle (adopted / disk_resolved / mutated /
    /// manifest_seen) driving the hand-off adoption precedence and the
    /// destructive save guard. Decisions live in `harpoon_core::bootstrap`;
    /// this is only the flag carrier.
    /// ACs: pane-pipe-api.respawn-state-hand-off,
    /// reorder.destructive-save-guard.
    boot: StoreBootState,
    /// One-shot guard against the re-delivered in-flight CLI invocation
    /// pipe (armed from the adopted payload's `handled_cli_pipe`).
    /// AC: pane-pipe-api.duplicate-toggle-delivery-tolerance.
    dup_guard: DuplicateToggleGuard,
    /// Live pane ids that disappeared while a frozen store was protected
    /// from shrinking. Core releases them only after disk+full-manifest
    /// readiness so normal pruning resumes exactly once.
    deferred_prunes: DeferredPruneGuard,
}

register_plugin!(State);

// ── ZellijPlugin trait impl ────────────────────────────────────────────────────

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Parse config; init matcher from it. Mode init at default_mode.
        // NO sync queries here: load() precedes the permission grant, and a
        // denied response-decoding query panics the plugin (2026-07-12
        // evidence). parked_tab_id is recorded at the Granted event — the
        // pane cannot change tab before then (relocation is forbidden).
        self.raw_configuration = configuration.clone();
        self.config = Config::parse_from_btree(&configuration);
        self.dispatch_state.default_mode = self.config.default_mode;
        self.dispatch_state.mode = self.config.default_mode;
        self.matcher = MatcherImpl::from_config(&self.config);

        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            // pane-pipe-api.host-call-permission-completeness: required for
            // unblock_cli_pipe_input / cli_pipe_output host calls.
            PermissionType::ReadCliPipes,
            // pane-pipe-api.host-call-permission-completeness: required for
            // open_plugin_pane_floating (the toggle respawn branch) — denied
            // response-decoding calls PANIC the plugin (2026-07-12).
            PermissionType::OpenTerminalsOrPlugins,
            PermissionType::MessageAndLaunchOtherPlugins,
        ]);
        subscribe(&[
            EventType::Key,
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::RunCommandResult,
            // Cold-show retry tick (pane-pipe-api.toggle-pipe-invocation):
            // suppressed panes receive no state events, so the cold-spawn
            // retry rides Event::Timer instead.
            EventType::Timer,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        // A cold-spawn toggle raced pane registration; any event (Timer is
        // the guaranteed one — suppressed panes get no state events) means
        // the host is alive — retry the show with fresh sync queries.
        if self.resolve_pending_cold_show() {
            should_render = true;
        }
        match event {
            Event::TabUpdate(tab_info) => {
                self.tab_info = Some(tab_info);
                self.refresh_manifest_readiness();
                self.update_panes(true);
                should_render = self.store_ready_to_render();
            }
            Event::PaneUpdate(pane_manifest) => {
                self.pane_manifest = Some(pane_manifest);
                self.refresh_manifest_readiness();
                self.update_panes(true);
                should_render = self.store_ready_to_render();
            }
            Event::PermissionRequestResult(status) => {
                if self.handle_permission_result(status) {
                    should_render = true;
                }
            }
            Event::SessionUpdate(session_infos, _) => {
                if self.session_name.is_none() {
                    if let Some(current) = session_infos.iter().find(|s| s.is_current_session) {
                        self.session_name = Some(current.name.clone());
                    }
                }
                self.start_disk_load_if_ready();
            }
            Event::RunCommandResult(_exit_code, stdout, _stderr, context) => {
                if context.get("source").map(|s| s.as_str()) == Some("load") {
                    let content = String::from_utf8_lossy(&stdout);
                    // The load RESOLVED (success or failure both count —
                    // reorder.destructive-save-guard readiness).
                    self.boot.disk_resolved = true;
                    // Adoption-vs-disk precedence is a core decision
                    // (pane-pipe-api.respawn-state-hand-off): a late disk
                    // result never clobbers an adopted payload or newer
                    // in-memory mutations.
                    match disk_load_decision(&self.boot) {
                        DiskLoadDecision::UseDisk => {
                            if let Err(e) =
                                self.persistence.on_load_command(&mut self.store, &content)
                            {
                                eprintln!("{e}");
                            }
                            // After resolution, attempt restore even for an
                            // explicit parse failure (status-quo empty UI).
                            self.update_panes(true);
                            should_render = self.store_ready_to_render();
                        }
                        DiskLoadDecision::MergeMissing => {
                            if let Some(disk) = Persistence::parse_content(&content) {
                                // Disk entries absent from memory append
                                // (index=None); memory mutations untouched.
                                merge_missing(&mut self.store.bookmarks, &disk);
                                self.persistence.set_baseline(disk);
                            }
                            // Explicit resolution flushes any queued user
                            // mutation even when content was malformed.
                            self.update_panes(true);
                            should_render = self.store_ready_to_render();
                        }
                        DiskLoadDecision::ReconcileBaseline => {
                            // Bootstrap is the sender's newer LIVE state;
                            // consume disk as reconciliation input/baseline
                            // without replacing or adjoining stale rows into
                            // newer memory.
                            if let Some(disk) = Persistence::parse_content(&content) {
                                self.persistence.set_baseline(disk);
                            }
                            self.update_panes(true);
                            should_render = self.store_ready_to_render();
                        }
                    }
                }
            }
            Event::Key(key) => {
                let input = key_event_to_input(&key);
                let ctx = self.build_dispatch_context();
                let effects = dispatch(&mut self.dispatch_state, &ctx, &mut self.store, input);
                // Stash filtered_indices for render path.
                self.last_filtered_indices = ctx.filtered_indices;
                self.apply_effects(&effects, &mut should_render);
            }
            _ => (),
        };
        should_render
    }

    /// External CLI-pipe interface (`pane-pipe-api` capability). Answers two
    /// named requests from `zellij pipe`, driven over the ntfy-harpoon-jump
    /// SSH side-channel:
    ///
    /// - `slot_for_pane` — reverse lookup; writes the 1-based harpoon slot for
    ///   the payload pane id (or an empty string) back to the CLI via
    ///   [`cli_pipe_output`]. Pure read; never mutates state.
    /// - `jump_pane` — focuses the payload pane id via the existing
    ///   deterministic fullscreen-safe [`State::jump_focus_fullscreen`].
    ///
    /// The payload is a zellij pane id (`terminal_N` as exported to
    /// `$ZELLIJ_PANE_ID`, or bare `N`). An unresolvable/absent payload is a
    /// no-op: no state mutation, no focus change.
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        // `toggle` is source-agnostic (the production caller is the keybind
        // `MessagePlugin` pipe — PipeSource::Keybind — probe-verified
        // 2026-07-11); the remaining names are CLI-facing surfaces.
        let is_cli = matches!(pipe_message.source, PipeSource::Cli(_));
        let is_plugin = matches!(pipe_message.source, PipeSource::Plugin(_));
        // Never call gated response-decoding/query/unblock hosts pre-grant.
        // Queue CLI messages (not keybind/plugin-source messages) and drain
        // after PermissionRequestResult(Granted). This preserves cold
        // jump_pane behavior without permission-denied panics.
        if is_cli && !self.permissions_granted {
            self.pending_cli_pipes.push(pipe_message);
            return false;
        }
        let cli_uuid = match &pipe_message.source {
            PipeSource::Cli(uuid) => Some(uuid.clone()),
            _ => None,
        };
        let payload = pipe_message.payload.unwrap_or_default();
        let mut should_render = false;
        match pipe_message.name.as_str() {
            // AC: pane-pipe-api.toggle-pipe-invocation
            "toggle" => {
                // AC pane-pipe-api.duplicate-toggle-delivery-tolerance:
                // zellij re-delivers the still-open CLI invocation pipe to a
                // respawned successor (~380ms, same uuid — probe 2026-07-13).
                // Identity match (armed from the adopted payload) ignores
                // exactly that one; the tail unblock below still releases
                // the CLI client.
                if self.dup_guard.is_stale_toggle(cli_uuid.as_deref()) {
                    should_render = false;
                } else {
                    should_render = self.handle_toggle(cli_uuid.as_deref());
                }
            }
            // AC: pane-pipe-api.respawn-state-hand-off — deny-safe pure-state
            // adoption (the payload arrives PRE-GRANT; no response-decoding
            // host calls in this arm or anything it calls).
            BOOTSTRAP_PIPE_NAME if is_plugin => {
                should_render = self.handle_bootstrap(&payload);
            }
            "slot_for_pane" if is_cli && self.permissions_granted => {
                let output = parse_pane_id(&payload)
                    .and_then(|id| slot_for_pane(&self.store, id))
                    .map(|slot| slot.to_string())
                    .unwrap_or_default();
                cli_pipe_output(&pipe_message.name, &output);
            }
            "jump_pane" if is_cli && self.permissions_granted => {
                if let Some(id) = parse_pane_id(&payload) {
                    self.jump_focus_fullscreen(id);
                }
            }
            _ => {}
        }
        // Always release a CLI client, exactly once, whatever the arm above
        // did (incl. the unrecognized-name no-op). The host's implicit release
        // proved racy on long-lived servers (2026-07-09 diagnosis: identical
        // back-to-back jump_pane pipes exited 0 then hung 124), stranding one
        // zombie `zellij pipe` process per ntfy tap. Non-CLI sources get no
        // CLI unblock (pane-pipe-api "non-CLI pipes unaffected").
        // AC: pane-pipe-api.cli-pipe-client-release
        if is_cli && self.permissions_granted {
            unblock_cli_pipe_input(&pipe_message.name);
        }
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        // Build row projection before the gate: core requires every adopted
        // bookmark to be present as live row or persisted placeholder, so a
        // bootstrap-before-manifest ordering cannot expose a partial list.
        let entries = build_row_entries(&self.dispatch_state, &self.store);
        if !store_ready_to_render(&self.boot, entries.len(), self.store.bookmarks.len()) {
            return;
        }
        let rows = rows as u16;
        let cols_u = cols;

        let budget = compute_layout_budget(rows);

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
    /// Core-owned first-render decision over the actual row projection.
    fn store_ready_to_render(&self) -> bool {
        let presented = build_row_entries(&self.dispatch_state, &self.store).len();
        store_ready_to_render(&self.boot, presented, self.store.bookmarks.len())
    }

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
    fn update_panes(&mut self, allow_host_save: bool) {
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

        // A frozen store may have lost a live pane while shrinking was
        // forbidden. Once disk + full-manifest readiness holds, core releases
        // only ids still absent; prune bookmark + sparse slot exactly once.
        let pruning_ready = shrinking_save_allowed(&self.boot);
        if pruning_ready {
            let visible_ids: Vec<u32> = valid_ids.iter().copied().collect();
            let deferred = self.deferred_prunes.take_prunable(&self.boot, &visible_ids);
            let mut slots = prune_bookmarks_by_pane_ids(&mut self.store, &deferred);
            slots.sort_unstable_by(|a, b| b.cmp(a));
            for slot in slots {
                if slot < self.dispatch_state.panes.len() {
                    self.dispatch_state.panes.remove(slot);
                }
            }
        }

        // Drop panes whose id is no longer valid. Pre-freeze (sparse) keeps
        // None placeholders untouched; post-freeze (dense) compacts the
        // disappearance.
        if !self.store.frozen || !pruning_ready {
            // Pre-freeze: turn invalid Some entries back into None.
            for opt in self.dispatch_state.panes.iter_mut() {
                if let Some(p) = opt.as_ref() {
                    if !valid_ids.contains(&p.id) {
                        // A post-freeze disappearance is a deferred prune,
                        // not a permanent unresolved placeholder.
                        if self.store.frozen {
                            self.deferred_prunes.remember(p.id);
                        }
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

        // Keep resolved bookmarks' persisted identity current with live
        // panes (titles are volatile — pi retitles continuously; the
        // on-disk fallback identity must be the freshest observed one).
        // AC: reorder.restore-identity-tracks-live-panes
        refresh_resolved_identities(&mut self.store, &visible);

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

        // Save if changed — through the destructive-save guard
        // (reorder.destructive-save-guard): reconcile pruning must never
        // shrink the disk file before this instance has observed both a
        // resolved baseline and a pane manifest.
        if allow_host_save {
            self.guarded_save();
        }
    }

    /// Focus the target terminal pane and deterministically leave it
    /// fullscreen — the always-on "jump fullscreens the target" behavior.
    ///
    /// zellij only exposes fullscreen *toggles* (no SetFullscreen), so the
    /// toggle decision must come from state that is true at decision time.
    /// Since zellij 0.44.3 the host exposes synchronous queries
    /// (`get_focused_pane_info`, `get_tab_info`), so nothing is predicted and
    /// no event cache is consulted:
    ///
    /// 1. focus the target pane by id (cross-tab capable);
    /// 2. query ground truth — the now-focused pane's tab and its
    ///    `TabInfo.is_fullscreen_active`;
    /// 3. hand the ground truth to core — [`harpoon_core::post_focus_fullscreen_plan`]
    ///    emits `[Effect::ToggleFullscreenPane(id)]` only when the tab is
    ///    provably tiled: from tiled, a toggle can only ENTER fullscreen, so
    ///    the wrong direction is structurally impossible — and the shim
    ///    applies the emitted effects (Constitution I split).
    ///
    /// Correct in all quadrants (plain/stacked × same-tab/cross-tab) and
    /// independent of the event caches, which are still `None` on a cold
    /// pipe-spawned instance (the ntfy notification-jump path receives its
    /// `PipeMessage` before the first `TabUpdate`/`PaneUpdate`).
    ///
    /// AC: `pane-pipe-api.jump-to-pane-by-id`
    /// AC: `pane-pipe-api.ground-truth-fullscreen-normalization`
    fn jump_focus_fullscreen(&mut self, id: u32) {
        focus_terminal_pane(id, true, false);

        let truth = match get_focused_pane_info() {
            Ok((tab_id, PaneId::Terminal(focused_id))) if focused_id == id => {
                match get_tab_info(tab_id) {
                    Some(tab) if tab.is_fullscreen_active => FullscreenGroundTruth::Fullscreen,
                    Some(_) => FullscreenGroundTruth::Tiled,
                    // Tab lookup failed: state unknown — never toggle blind.
                    None => FullscreenGroundTruth::Unknown,
                }
            }
            // Focus did not land on the target (pane gone, query error):
            // state unknown — never toggle blind.
            _ => FullscreenGroundTruth::Unknown,
        };

        let mut ignored_render = false;
        let plan = post_focus_fullscreen_plan(truth, id);
        self.apply_effects(&plan, &mut ignored_render);
    }

    /// Apply a `Vec<Effect>` from the dispatch core — the single shim-side
    /// mapping from core-emitted effects to zellij host calls. `Effect::Close`
    /// triggers `hide_self()` + close-helper reset; `Effect::FocusPane(id)`
    /// triggers [`State::jump_focus_fullscreen`];
    /// `Effect::ToggleFullscreenPane(id)` calls
    /// `toggle_pane_id_fullscreen(PaneId::Terminal(id))`; `Effect::Save`
    /// saves persistence; `Effect::Render` flips `should_render = true`;
    /// `Effect::Noop` is ignored.
    ///
    /// Order is significant for `Close ↔ FocusPane`: handlers MUST emit them
    /// as `[Close, FocusPane]` so `hide_self()` runs before
    /// `jump_focus_fullscreen()` — the plugin pane leaves the screen first,
    /// then the focus/fullscreen actions on the target terminal pane are the
    /// last focus-affecting actions (they target terminal panes by id, so the
    /// hidden plugin pane can never steal the final focus).
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
                    self.jump_focus_fullscreen(*id);
                }
                Effect::ToggleFullscreenPane(id) => {
                    toggle_pane_id_fullscreen(PaneId::Terminal(*id));
                }
                Effect::Save => {
                    // A Save effect means the user mutated the store — from
                    // here on, bootstrap/disk data is older than memory
                    // (pane-pipe-api.respawn-state-hand-off precedence).
                    self.boot.mutated = true;
                    self.guarded_save();
                }
                Effect::Noop => {}
            }
        }
    }

    /// Single canonical close path: `hide_self()`, reset mode to default,
    /// clear query, re-anchor selected so the next open lands on a valid
    /// index.
    ///
    /// `hide_self()` (vs `close_self()`) keeps the plugin instance alive so
    /// the next `LaunchOrFocusPlugin` re-shows it warm — no per-invocation
    /// wasm load (~47–92ms) and no cold event caches. This reverts commit
    /// d6a2039, whose close_self workaround targeted a zellij mis-focus quirk
    /// (re-focusing a hidden floating plugin pane landed focus on a terminal
    /// pane); the quirk does not reproduce on zellij 0.44.3, the supported
    /// floor (verified 2026-07-09: 10/10 hide/relaunch cycles under the
    /// original trigger condition, one plugin load). This also rejoins the
    /// mode-state-machine spec, which mandates `hide_self()` on this path.
    fn close_helper(&mut self) {
        // No parked-tab recording needed here: hiding never moves the pane,
        // so the load-time record stays authoritative. (Also deliberate:
        // this path runs in Key-event/update context — keep synchronous
        // host queries out of it.)
        hide_self();
        self.dispatch_state.mode = self.dispatch_state.default_mode;
        self.dispatch_state.query.clear();
        let f_idx = core_focused_idx(
            &self.dispatch_state.panes,
            self.dispatch_state.focused_pane_id,
        );
        reanchor_selected_to_focus(&mut self.dispatch_state, f_idx);
    }

    /// `toggle` pipe handler: gather ground truth via SYNCHRONOUS host
    /// queries (never cached events — probes proved `TabUpdate`/`PaneUpdate`
    /// caches freeze while suppressed and `Event::Visible` is never delivered
    /// to floating plugin panes), let core pick the branch, execute it.
    /// Returns whether a re-render is warranted (we just became visible).
    ///
    /// Consume the aggregate permission result. Zellij 0.44.3 returns one
    /// status for the requested vector and blocks all normal plugin events
    /// while any request is unresolved; it exposes no per-permission result.
    /// Runtime probes confirmed sequential/nested requests leave the plugin
    /// permission-modal. Therefore spawn + hand-off capabilities are one
    /// verified aggregate. Aggregate denial stays deny-safe and shows in
    /// place; aggregate grant enables both response-decoding calls.
    fn handle_permission_result(&mut self, status: PermissionStatus) -> bool {
        if status == PermissionStatus::Granted {
            self.permissions_granted = true;
            self.handoff_permission_granted = true;
            // Earliest safe moment for response-decoding sync queries.
            self.record_parked_if_self_focused();
            let plugin_ids = get_plugin_ids();
            rename_plugin_pane(plugin_ids.plugin_id, "harpoon");
            // A bootstrap may have supplied the session name pre-grant;
            // start exactly one disk reconciliation now.
            self.start_disk_load_if_ready();
            let pending = std::mem::take(&mut self.pending_cli_pipes);
            let mut should_render = false;
            for message in pending {
                should_render |= self.pipe(message);
            }
            should_render
        } else {
            // Denied aggregate capabilities cannot disk-load, but this is a
            // terminal readiness outcome: render status-quo empty UI rather
            // than suppress output forever. No gated host call follows.
            self.boot.permission_denied = true;
            self.pending_cold_show = false;
            // Aggregate denial includes ChangeApplicationState: zellij
            // tears down the prompt, suppresses the plugin, and ignores a
            // post-denial show_self (runtime-proven). Stay inert/deny-safe;
            // the user's terminal remains visible and the plugin survives.
            true
        }
    }

    /// Start the successor/cold-boot disk load exactly once, once BOTH the
    /// session name and baseline RunCommands grant exist. Session name may
    /// arrive from SessionUpdate OR pre-grant bootstrap; tracking initiation
    /// separately prevents bootstrap from suppressing reconciliation.
    fn start_disk_load_if_ready(&mut self) {
        if self.permissions_granted && !self.disk_load_started && self.session_name.is_some() {
            self.disk_load_started = true;
            self.persistence.load_from_disk(&self.session_name);
        }
    }

    /// Set the destructive-save manifest half only when PaneUpdate covers
    /// every tab currently known by TabUpdate. This moves "full" from a
    /// non-empty shim proxy to a pure, natively-tested core predicate.
    fn refresh_manifest_readiness(&mut self) {
        let (Some(manifest), Some(tabs)) = (&self.pane_manifest, &self.tab_info) else {
            return;
        };
        let manifest_positions: Vec<usize> = manifest.panes.keys().copied().collect();
        let tab_positions: Vec<usize> = tabs.iter().map(|t| t.position).collect();
        if manifest_covers_tabs(&manifest_positions, &tab_positions) {
            self.boot.manifest_seen = true;
        }
    }

    /// Adopt a `bootstrap_store` hand-off payload from a predecessor
    /// instance. Pure state only — the payload arrives before this
    /// instance's permission grant (probe 2026-07-13), so ANY
    /// response-decoding host call here would panic on denial.
    /// AC: pane-pipe-api.respawn-state-hand-off
    fn handle_bootstrap(&mut self, raw: &str) -> bool {
        // Deny-safe decode: malformed/foreign/wrong-version → keep the
        // existing disk-load path untouched.
        let Some(payload) = decode_bootstrap(raw) else {
            return false;
        };
        match bootstrap_arrival_decision(&self.boot) {
            AdoptDecision::Ignore => false,
            AdoptDecision::Adopt => {
                // Every id in a targeted hand-off was live/resolved state in
                // the predecessor's session generation. Enrol it before the
                // successor clears materialization: once disk + full manifest
                // are ready, absent ids resume normal prune rather than
                // becoming permanent ghosts.
                for pane_id in payload.bookmarks.iter().filter_map(|b| b.id) {
                    self.deferred_prunes.remember(pane_id);
                }
                self.store.bookmarks = payload.bookmarks.clone();
                self.store.pane_id_to_bookmark_idx.clear();
                // Force a clean restore round against the adopted set (ids
                // are valid — same zellij session as the sender:
                // reorder.restore-identity-tracks-live-panes).
                self.dispatch_state.panes.clear();
                // Bootstrap payload is the sender's last live persisted
                // shape: safe baseline for classifying additive/reorder vs
                // shrink during the hand-off window. It NEVER substitutes
                // for disk_resolved in shrinking_save_allowed; the actual
                // disk result later replaces this comparison baseline.
                self.persistence.set_baseline(payload.bookmarks.clone());
                if self.session_name.is_none() {
                    self.session_name = payload.session_name;
                }
                self.dup_guard.arm(payload.handled_cli_pipe);
                self.boot.adopted = true;
                // Pure cached-state restore NOW (no host save): probe order
                // is PaneUpdate → bootstrap → grant, so waiting for another
                // event creates an empty/resolving first render. If either
                // cache is absent update_panes is a deny-safe no-op; a later
                // event resolves it.
                self.update_panes(false);
                // Cached-grant ordering is possible: if bootstrap supplied
                // the first session name after grant, initiate reconciliation
                // now. Pre-grant this helper is a pure no-op.
                self.start_disk_load_if_ready();
                self.store_ready_to_render()
            }
        }
    }

    /// Guarded save: shrinking saves (dropping a bookmark present in the
    /// last persisted state) are forbidden until BOTH a resolved disk load
    /// AND a full pane manifest have been observed. With NO known baseline,
    /// the user mutation remains allowed in memory but disk flush is queued
    /// (deferred fail-closed — the load reconciliation's update_panes flushes
    /// it once the baseline resolves). AC: reorder.destructive-save-guard
    fn guarded_save(&mut self) {
        match guarded_save_decision(
            &self.boot,
            self.persistence.last_persisted(),
            &self.store.bookmarks,
        ) {
            GuardedSaveDecision::Save => self
                .persistence
                .save_if_changed(&self.store, &self.session_name),
            GuardedSaveDecision::Defer => {}
        }
    }

    /// AC: pane-pipe-api.toggle-pipe-invocation
    /// AC: pane-pipe-api.toggle-state-sync-query-verified
    fn handle_toggle(&mut self, cli_uuid: Option<&str>) -> bool {
        if !self.permissions_granted {
            // Pre-grant: response-decoding queries would panic the plugin
            // if denied. Deny-safe attempt + timer retry (the grant event
            // arrives within moments on a cold spawn).
            show_self(true);
            self.pending_cold_show = true;
            self.cold_show_retries = 25;
            set_timeout(0.2);
            return true;
        }
        let own_id = get_plugin_ids().plugin_id;
        let own_pane_id = zellij_tile::prelude::PaneId::Plugin(own_id);
        // Fresh own-pane state: Some(is_suppressed) or None (query failed /
        // cold-spawn window before the pane registers host-side). NOTE:
        // is_suppressed=false does NOT mean user-visible — a cold-spawned
        // pane parks floating+unfocused; the focused-pane identity below is
        // the "open in front of the user" signal.
        let own_pane = get_pane_info(own_pane_id);
        let own_suppressed = own_pane.as_ref().map(|p| p.is_suppressed);
        // One sync query serves both: the focused pane identity and the
        // invoking tab (get_focused_pane_info returns the STABLE TAB ID —
        // zellij screen active_tab_ids).
        let focused = get_focused_pane_info().ok();
        let own_is_focused = focused.as_ref().map(|(_, pane_id)| *pane_id == own_pane_id);
        let parked_on_focused_tab = match (focused.as_ref(), self.parked_tab_id) {
            (Some((focused_tab_id, _)), Some(parked)) => Some(*focused_tab_id == parked),
            _ => None,
        };
        match toggle_plan(ToggleGroundTruth {
            own_suppressed,
            own_is_focused,
            parked_on_focused_tab,
        }) {
            ToggleAction::Hide => {
                // This branch fires only when own_is_focused verified true —
                // the focused tab IS our tab: refresh the parked record
                // (identity-verified, not a proxy) before hiding.
                if let (Some((tab_id, _)), Some(true)) = (focused.as_ref(), own_is_focused) {
                    self.parked_tab_id = Some(*tab_id);
                }
                // Same canonical close path as Esc (mode-state-machine
                // "Close consolidation" — unchanged semantics).
                self.close_helper();
                false
            }
            ToggleAction::ShowInPlace => {
                // Position-correct host path (screen.focus_pane_with_id):
                // finds the pane's tab including suppressed panes, navigates
                // by tab.position, un-suppresses back to floating.
                show_self(true);
                // Post-show we should hold focus on our own tab — refresh
                // the record under identity verification.
                self.record_parked_if_self_focused();
                true
            }
            ToggleAction::Respawn => {
                // Owner-ruled mechanism (decision-audit 2026-07-11): open a
                // FRESH instance of ourselves floating on the invoking tab
                // (a new-pane host action — never the broken
                // focus/relocation paths; break_panes_to_tab_with_index
                // DESTROYS the pane under tab-id/position drift, upstream
                // defect #3), then close this instance. The fresh instance
                // reuses our verbatim URL + configuration so the keybind's
                // pipe keeps reaching it.
                let own_url = own_pane.as_ref().and_then(|p| p.plugin_url.clone());
                match own_url {
                    Some(url) => {
                        let spawned = open_plugin_pane_floating(
                            &url,
                            self.raw_configuration.clone(),
                            None,
                            BTreeMap::new(),
                        );
                        match spawned {
                            Some(PaneId::Plugin(new_id)) => {
                                // Targeted bootstrap hand-off
                                // (pane-pipe-api.respawn-state-hand-off):
                                // ship the live store to the successor by
                                // destination plugin id — never url+config
                                // matching (would also deliver back to us) —
                                // BEFORE closing. Send failure loses only
                                // the hand-off: the successor falls back to
                                // its own disk load.
                                let payload = BootstrapPayload {
                                    version: BOOTSTRAP_VERSION,
                                    bookmarks: self.store.bookmarks.clone(),
                                    session_name: self.session_name.clone(),
                                    handled_cli_pipe: cli_uuid.map(str::to_string),
                                };
                                if self.handoff_permission_granted {
                                    if let Some(encoded) = encode_bootstrap(&payload) {
                                        pipe_message_to_plugin(
                                            MessageToPlugin::new(BOOTSTRAP_PIPE_NAME)
                                                .with_destination_plugin_id(new_id)
                                                .with_payload(encoded),
                                        );
                                    }
                                }
                                // No hand-off grant → skip the gated send;
                                // successor's independently-started disk load
                                // is the required fallback.
                                close_self();
                                false
                            }
                            Some(_) => {
                                // Spawn succeeded but the id is not a
                                // plugin pane — skip the hand-off (successor
                                // disk-loads) and close as today.
                                close_self();
                                false
                            }
                            None => {
                                // Frozen delta explicitly classifies a
                                // missing affected-pane id as "id unavailable"
                                // rather than proof no successor exists:
                                // skip hand-off, close predecessor, let any
                                // spawned successor cold-load from disk.
                                close_self();
                                false
                            }
                        }
                    }
                    None => {
                        // No URL available — same safe degradation.
                        show_self(true);
                        true
                    }
                }
            }
            ToggleAction::ColdShow => {
                // Cold-spawn window: the pipe can precede host-side pane
                // registration, making this show_self a possible no-op
                // (regression run 2026-07-11). Attempt it (harmless either
                // way) and arm the timer-driven retry — suppressed panes
                // receive no TabUpdate/PaneUpdate, so a timer is the only
                // reliable wake-up.
                show_self(true);
                self.pending_cold_show = true;
                self.cold_show_retries = 25; // 25 × 0.2s = 5s budget
                set_timeout(0.2);
                true
            }
        }
    }

    /// Resolve an armed cold-spawn show at event arrival: re-query the own
    /// pane FRESH; once it exists, show it (and relocate to the freshly
    /// queried focused tab — the invoking tab; the user cannot have moved in
    /// the ~tens-of-ms registration window). Never reads cached event
    /// content. Returns whether a show was issued.
    fn resolve_pending_cold_show(&mut self) -> bool {
        if !self.pending_cold_show {
            return false;
        }
        if !self.permissions_granted {
            // Still pre-grant — queries would panic if denied; re-arm.
            if self.cold_show_retries > 0 {
                self.cold_show_retries -= 1;
                set_timeout(0.2);
            } else {
                self.pending_cold_show = false;
            }
            return false;
        }
        let own_id = get_plugin_ids().plugin_id;
        let own_pane = get_pane_info(zellij_tile::prelude::PaneId::Plugin(own_id));
        let Some(pane) = own_pane else {
            // Still unregistered — re-arm the timer within budget.
            if self.cold_show_retries > 0 {
                self.cold_show_retries -= 1;
                set_timeout(0.2);
            } else {
                self.pending_cold_show = false; // budget exhausted — give up
            }
            return false;
        };
        self.pending_cold_show = false;
        let _ = pane;
        // show_self is the position-correct focus path (harmless re-focus
        // if the initial attempt won the race); then record the parked tab
        // under identity verification (post-grant context — queries safe).
        show_self(true);
        self.record_parked_if_self_focused();
        true
    }

    /// Record `parked_tab_id` ONLY when the client's focused pane is
    /// verifiably our own pane — then the focused tab equals our tab by
    /// identity (never a proxy; round-2 P1: a focused-tab sample while a
    /// jump target holds focus poisons the record and re-creates the
    /// wrong-tab symptom). On any mismatch/failure the record is left
    /// unchanged; an unknown record degrades to the safe Respawn branch.
    /// Caller must ensure permissions are granted (response-decoding query).
    fn record_parked_if_self_focused(&mut self) {
        let own_id = get_plugin_ids().plugin_id;
        if let Ok((tab_id, pane_id)) = get_focused_pane_info() {
            if pane_id == zellij_tile::prelude::PaneId::Plugin(own_id) {
                self.parked_tab_id = Some(tab_id);
            }
        }
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
