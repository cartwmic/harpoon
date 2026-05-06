## 0. Phase 0 — Verification spikes (must pass before any production work)

- [ ] 0.1 **Render coordinate verification**: render a probe string at `y=0` and `y=1` in the dev workspace; visually confirm whether `y=0` is hidden (per existing `// Note: y=0 overlaps with the zellij pane frame/title bar` comment in `src/main.rs`). Record the answer in design.md and in code as a constant `HEADER_BASE_Y: u16` (0 if visible, 1 if hidden)
- [ ] 0.2 **Plugin instance lifecycle verification**: add `eprintln!("instance addr: {:p}", self as *const _)` in both `load()` and `update()`; cycle open/close/open in the dev workspace and confirm the same address (i.e. `State` survives `hide_self`). If the assumption fails, the close-helper reset becomes dead code and the design must move mode-init to `load()` — escalate before continuing
- [ ] 0.3 **`BareKey::Char('K')`/`('J')` shift-detection probe**: add an `eprintln!` that logs the exact `Key` event received in command mode; press shift-k and shift-j; confirm the host emits `BareKey::Char('K')` not `Char('k')`+shift modifier (consistent with the existing `Char('A')` usage). Remove the probe before merging
- [ ] 0.4 **`nucleo-matcher` wasm + index-semantics spike**: add `nucleo-matcher` to a throwaway branch with `--no-default-features` plus the minimum needed for `Matcher::fuzzy_indices`; run `cargo build --target wasm32-wasip1`. In the same branch, write a unit test that feeds haystack `"📦 build"` and needle `"b"` through the matcher's `Utf32Str`/`Utf32String` constructor and asserts the returned index is `2` (char position) NOT `4` (byte offset). Decide fuzzy backend based on results; if char-index semantics or wasm build fails, switch to in-tree matcher
- [ ] 0.5 **Render-side multi-byte highlight verification (STOP GATE for highlight work)**: in the dev workspace, render `Text::new("📦 build")` followed by `Text::color_range(1, 2..3)` (and separately `Text::color_indices(1, vec![2])`) at a known coordinate. Visually confirm that the `b` glyph (NOT the `📦` or trailing space) is highlighted. This verifies the **host** decoder treats indices as char positions rather than byte offsets. If the host indexes by byte, all match-highlight code in `harpoon-core` must convert char→byte at the FFI render shim before passing to `Text` (using `s.char_indices().nth(char_idx).map(|(b, _)| b)`); update the render layer tasks accordingly. Mark Phase 0 as a stop gate for any highlight work pending this result

## 1. Workspace restructure & foundation

- [ ] 1.1 Convert root `Cargo.toml` to a workspace with two members: `harpoon-core` (new lib, no `zellij-tile` dep, native+wasm) and `harpoon-plugin` (existing crate moved into `harpoon-plugin/`, cdylib, depends on `harpoon-core` and `zellij-tile`). The cdylib package name remains `harpoon` so the built artifact path stays `target/wasm32-wasip1/release/harpoon.wasm`
- [ ] 1.1b **Move target pin**: relocate `.cargo/config.toml` (with its `wasm32-wasip1` target pin) from the repo root into `harpoon-plugin/.cargo/config.toml`. Workspace root has NO target pin so `cargo test -p harpoon-core` from root runs natively. **Important**: cargo walks `.cargo/config.toml` from the cwd upward, NOT from the package directory — so `cargo build -p harpoon` from workspace root will NOT pick up the wasm pin. Plugin builds must either `cd harpoon-plugin/ && cargo build --release` OR `cargo build -p harpoon --target wasm32-wasip1 --release` from root with explicit `--target`
- [ ] 1.2 Define **host-agnostic projection types** in `harpoon-core/src/lib.rs`:
  ```rust
  pub struct Pane { pub id: u32, pub tab_name: String, pub pane_title: String, pub tab_position: u32 }
  pub struct ModifierSet { pub ctrl: bool, pub alt: bool, pub shift: bool, pub super_: bool }
  pub enum InputKey { Char(char, ModifierSet), Backspace, Esc, Enter, ArrowUp, ArrowDown, Other }
  impl ModifierSet { pub fn is_plain_or_shift(&self) -> bool { !self.ctrl && !self.alt && !self.super_ } }
  ```
  Move the `Display` impl for `Pane` to `harpoon-core`. Plugin keeps `zellij_tile::PaneInfo`/`TabInfo` and converts at the FFI boundary (separate task in section 3)
- [ ] 1.3 In `harpoon-core`, create `mode.rs` with `enum Mode { Command, Filter, Jump }` and parse helpers (case-insensitive)
- [ ] 1.4 In `harpoon-core`, create `config.rs` with `Config { default_mode, matcher, show_slots }` and `parse_from_btree(map: &BTreeMap<String,String>) -> Config` honoring fallback rules; use `eq_ignore_ascii_case` for `default_mode`, `matcher`, AND `show_slots`
- [ ] 1.5 In `harpoon-core`, create `effect.rs` with `enum Effect { Render, Close, FocusPane(u32), Save, Noop }`. Document that effects in a `Vec<Effect>` are applied in declared order
- [ ] 1.6 Verify `cargo test -p harpoon-core` from workspace root compiles and runs an empty test using the host's default native target. This validates the workspace + target-pin split before any tests depend on it
- [ ] 1.7 Add fields to the plugin's `State`: `mode: Mode`, `default_mode: Mode`, `query: String`, `config: Config`, `matcher: MatcherImpl` (NOT `Box<dyn Matcher>` — see decision in design.md). `MatcherImpl` derives `Default` so `State` keeps `#[derive(Default)]`
- [ ] 1.8 In `ZellijPlugin::load`: after `Config::parse_from_btree` populates `state.config`, set `state.matcher = MatcherImpl::from_config(&state.config)`. Add a unit test (in `harpoon-core` if `from_config` is fully testable there, else `harpoon-plugin/tests/`) asserting that `from_config(&Config { matcher: Substring, .. })` returns the `MatcherImpl::Substring(_)` variant

## 2. Matcher (TDD, static dispatch)

- [ ] 2.1 In `harpoon-core/src/matcher.rs`, define `pub trait Matcher { fn match_indices(&mut self, haystack: &str, needle: &str) -> Option<(i32, Vec<usize>)>; }` returning `(score, char_indices)` where indices are **character (Unicode scalar) positions**. Use `&mut self` to accommodate `nucleo::Matcher`'s internal scratch buffers; in-tree fallback ignores the mutability
- [ ] 2.2 Write failing tests FIRST (`cargo test -p harpoon-core matcher::`): empty needle returns `Some((max_score, vec![]))`; no-match returns `None`; ASCII haystack returns char indices; multi-byte haystack `"📦 build"` with needle `"b"` returns `[2]` not `[4]`; case-insensitive matching; score ordering puts contiguous matches above scattered
- [ ] 2.3 Implement `FuzzyMatcher` to make tests pass (wrapping `nucleo-matcher` if Phase 0 spike confirmed; else in-tree subseq matcher iterating `haystack.char_indices()` and recording char positions). `FuzzyMatcher` derives `Default`
- [ ] 2.4 Implement `SubstringMatcher` (case-insensitive ASCII fold; returns contiguous char-index range as the index list). Derives `Default`
- [ ] 2.5 Define `pub enum MatcherImpl { Fuzzy(FuzzyMatcher), Substring(SubstringMatcher) }` with `Default = Fuzzy(FuzzyMatcher::default())` and a `match_indices` impl dispatching to the inner matcher. Add `pub fn from_config(config: &Config) -> MatcherImpl` constructor
- [ ] 2.6 Run `cargo test -p harpoon-core` and confirm all matcher tests pass before moving on

## 3. Pure dispatch core & FFI conversion (TDD)

- [ ] 3.1 In `harpoon-core`, define:
  ```rust
  pub struct DispatchState {
      pub mode: Mode,
      pub default_mode: Mode,
      pub query: String,
      pub panes: Vec<Option<Pane>>,    // sparse during partial restore; dense post-first-mutation
      pub selected: usize,             // indexes panes; may land on a None (placeholder)
      pub focused_pane_id: Option<u32>,
  }
  pub struct DispatchContext { pub focused_pane: Option<Pane>, pub visible_panes: Vec<Pane>, pub filtered_indices: Vec<usize> }
  ```
  `DispatchState.panes` is `Vec<Option<Pane>>` to preserve saved-position slot mapping during the partial-restore window. `Some(p)` = live pane at this slot index; `None` = placeholder slot (an unresolved bookmark with `index = Some(i)`). `DispatchContext.filtered_indices` is the score-ordered filtered view (Live panes only; placeholders excluded from filter). Handlers mutate `&mut DispatchState` and read `&DispatchContext`; never call FFI
- [ ] 3.2 Implement `pub fn dispatch(state: &mut DispatchState, ctx: &DispatchContext, key: InputKey) -> Vec<Effect>` that matches on `state.mode` and delegates to `handle_command_key(state, ctx, key)`, `handle_filter_key(state, ctx, key)`, `handle_jump_key(state, ctx, key)`. **All three handlers take `ctx`** for signature uniformity: command needs `ctx.focused_pane` for `a` and `ctx.visible_panes` for `A`; filter needs `ctx.focused_pane` for re-anchor on Esc-clear and `ctx.filtered_indices` for `Enter`/arrow nav; jump needs `ctx` only to detect placeholder slots (looking up resolved-state via the bookmarks-aware `RowEntry` Vec; see task 7.x). Each handler returns `Vec<Effect>`
- [ ] 3.3 Implement helpers in `harpoon-core`:
  - `pub fn reanchor_selected_to_focus(state: &mut DispatchState, focused_idx: Option<usize>)`: applies the gate `mode == Command || (mode == Filter && query.is_empty())` and, when the gate passes, sets `selected = focused_idx.unwrap_or(0).min(panes.len().saturating_sub(1))`. On gate-fail, `selected` is unchanged.
  - `pub fn clamp_selected_to_view(state: &mut DispatchState, view_len: usize)`: sets `selected = selected.min(view_len.saturating_sub(1))`. Mode-agnostic. Caller passes the current visible view length (`panes.len()` for command/jump; `filtered_indices.len()` for filter mode with non-empty query).
  - `pub fn focused_idx(panes: &[Option<Pane>], focused_pane_id: Option<u32>) -> Option<usize>`: returns the index `i` such that `panes[i] == Some(p)` and `p.id == focused_pane_id`, or `None` if the focused pane id is not present in `panes` (or `focused_pane_id` is `None`). Single canonical helper used by `update_panes`, `close_helper`, filter Esc-clear, and backspace-to-empty re-anchor
- [ ] 3.4 Plugin shim FFI conversion. Implement:
  - `fn pane_info_to_pane(info: &PaneInfo, tab_name: &str, tab_position: u32) -> harpoon_core::Pane` (conversion at the FFI boundary; PaneInfo doesn't carry tab_name)
  - `fn key_event_to_input(key: zellij_tile::Key) -> harpoon_core::InputKey` mapping `BareKey::Char(c)` + modifiers to `InputKey::Char(c, ModifierSet)`. **Apply ASCII letter normalization**: if `c.is_ascii_lowercase() && shift_modifier_set`, rewrite to `Char(c.to_ascii_uppercase()) + ModifierSet { shift: false, .. }`. This makes `K`/`J` and other shift+letter inputs match a canonical uppercase-no-shift form regardless of host emission style
  - `fn build_dispatch_context(&self) -> DispatchContext`: from `pane_manifest` and `tab_info`, build `visible_panes` sorted by (tab.position ASC, PaneInfo.id ASC), and resolve `focused_pane` from `focused_pane: Option<Pane>` field
- [ ] 3.5 Plugin shim: `update()` collects events, converts `Event::Key` to `InputKey`, calls `dispatch(...)`, then applies effects in declared order: `Effect::Close` → `self.close_helper()`; `Effect::FocusPane(id)` → `focus_terminal_pane(id, true)`; `Effect::Save` → `self.persistence.save_if_changed()` (no-arg form; `Persistence::bookmarks` is the canonical disk shape, not rebuilt from `panes`); `Effect::Render` → set `should_render = true`; `Effect::Noop` → ignored
- [ ] 3.5b **Non-Key events preserve `should_render` AND `has_changed` save**: `Event::PaneUpdate`, `Event::TabUpdate`, `Event::RunCommandResult`, `Event::SessionUpdate`, etc. continue to set `should_render = true` on their existing paths. They also call `self.persistence.save_if_changed()` (no-arg form) at the tail of their handler. This preserves the today-implicit save-on-pane-event-mutation, which `Effect::Save` does NOT cover (Effects only fire from key dispatch). Only `Event::Key` flows through `dispatch` + `Effect` application; `Effect::Save` is the additional save trigger for key-driven mutations.

  **Bookmark sync responsibility**: every non-Key handler that mutates `state.panes` (e.g. restore resolution in `update_panes`) MUST also keep `Persistence::bookmarks` in sync per the rules in design.md "Persistence has_changed covers full persisted shape" — specifically: when a bookmark resolves and a `Pane` is inserted into `state.panes`, the bookmark stays in `Persistence::bookmarks` unchanged. When freeze rewrites unresolved `Some(_)` → `None`, the bookmarks Vec is mutated in place. `save_if_changed()` then reads the canonical `bookmarks` Vec and writes the envelope
- [ ] 3.6 Plugin shim: `close_helper(&mut self)` calls `hide_self()`, sets `self.mode = self.default_mode`, clears `self.query`, then calls `reanchor_selected_to_focus(&mut self.dispatch_state, focused_idx(&self.dispatch_state.panes, self.dispatch_state.focused_pane_id))` to re-anchor `selected` immediately (no longer relying on a subsequent `PaneUpdate`). View-length clamp via `clamp_selected_to_view` is the next render path's responsibility — close itself only re-anchors
- [ ] 3.7 Replace every direct `hide_self()` callsite with `Effect::Close` returns from handlers (effects flow through `close_helper` in 3.5)
- [ ] 3.8 In `harpoon-core`: `impl DispatchState { pub fn selected_pane_index(&self, view: &[usize]) -> Option<usize> { ... } }`. Returns the underlying `panes` index for the current `selected`: in command/jump mode, `view` is `(0..panes.len()).collect()` so result is `selected`; in filter mode with non-empty query, `view` is `ctx.filtered_indices`. Plugin shim is a thin pass-through: `fn selected_pane_index(&self) -> Option<usize> { self.dispatch_state.selected_pane_index(&self.last_filtered_indices) }`
- [ ] 3.9 Write failing tests FIRST for dispatch core: `dispatch` returns `vec![Effect::Render]` for mode transitions; `dispatch` returns `[Effect::Close, Effect::FocusPane(id)]` (in that order) for jumps and Enter; `reconcile_selected` honors the mode gate; `reconcile_selected` clamps to `panes.len() - 1`; `Effect::Save` only emitted from mutating paths
- [ ] 3.10 Write failing tests FIRST for FFI conversion via the primitive shim `harpoon_core::parse_input_key(...)` (lives in `harpoon-core`, native-testable). Tests live in `harpoon-core/src/key.rs`'s `#[cfg(test)] mod tests`: `parse_input_key("Char", Some('a'), false, false, false, false)` → `InputKey::Char('a', empty)`; `parse_input_key("Char", Some('a'), true, false, false, false)` → `InputKey::Char('a', {ctrl})`; `parse_input_key("Char", Some('k'), false, false, true, false)` → `InputKey::Char('K', empty)` (shift normalization); `parse_input_key("Char", Some('K'), false, false, false, false)` → `InputKey::Char('K', empty)` (already canonical); `parse_input_key("Backspace", None, ...)` → `InputKey::Backspace`. Plugin's `From<zellij_tile::Key>` is a thin one-line wrapper delegating to this primitive shim
- [ ] 3.11 Run `cargo test -p harpoon-core dispatch::` and confirm all dispatch tests pass before moving on

## 4. Command mode

- [ ] 4.0 Implement `pub fn freeze_on_user_mutation(state: &mut DispatchState, persistence: &mut Persistence)` in `harpoon-core` (or the plugin shim if `Persistence` lives there — helper signature stays the same). Algorithm per design.md "State.panes is a sparse Vec":
  1. Capture `prev_selected_pane_id` from `state.panes[state.selected].as_ref().map(|p| p.id)`
  2. Rewrite all unresolved bookmarks (those whose `pane.id` is NOT a key in `persistence.pane_id_to_bookmark_idx`) with `index = Some(_)` to `index = None`
  3. Compact: `state.panes.retain(|opt| opt.is_some())`
  4. Rebuild `persistence.pane_id_to_bookmark_idx` and update each surviving bookmark's `index = Some(new_dense_position)`. Use a per-pass `HashMap<(String, String), Vec<usize>>` lookup keyed on `(tab_name, pane_title)` to avoid quadratic scan with duplicate-titled panes
  5. Re-anchor `state.selected`: if `prev_selected_pane_id` was `Some(pid)`, set `selected` to `panes.iter().position(|opt| opt.as_ref().map(|p| p.id) == Some(pid)).unwrap_or(0)`; else `selected = 0`

  Write failing unit tests FIRST: freeze with all-resolved leaves panes unchanged; freeze with one None placeholder compacts to dense; freeze rebuilds map correctly with duplicate-titled panes; freeze re-anchors selection to the previously-selected pane id post-compaction; freeze with selection-on-None lands at index 0

- [ ] 4.1 Implement `handle_command_key(state, ctx, key)`. Each branch enumerates its effects explicitly. ASCII-letter and digit arms gate on `modifiers.is_plain()` (post-normalization); modified inputs return `vec![Effect::Noop]` UNLESS noted otherwise. Symbol-key arms (`/`, `#`) accept any modifier set:
    - `Char('a', plain)`: if `ctx.focused_pane` is Some AND its id is not already a key in `persistence.pane_id_to_bookmark_idx`: **call `freeze_on_user_mutation(state, persistence)` first**, THEN push `Some(focused_pane)` to `state.panes`, THEN push `PaneBookmark { tab_name, pane_title, index: Some(state.panes.len() - 1) }` to `persistence.bookmarks`, THEN insert `pane.id → bookmarks.len() - 1` into `persistence.pane_id_to_bookmark_idx`, return `[Effect::Save, Effect::Render, Effect::Close]`. If `ctx.focused_pane` is None or pane already present, return `vec![]` (no freeze)
    - `Char('A', plain)` (post-normalization): if any visible pane in `ctx.visible_panes` is NOT already in `persistence.pane_id_to_bookmark_idx`: **call `freeze_on_user_mutation(state, persistence)` first**, THEN iterate `ctx.visible_panes` (already sorted) and for each pane not present, push `Some(pane)` to `state.panes` and a matching `PaneBookmark` with `index = Some(new_idx)` plus map entry. Return `[Effect::Save, Effect::Render]` — **NO `Effect::Close`**. If every visible pane is already pinned, return `vec![]` (no freeze, no-op)
    - `Char('d', plain)`: if `state.panes.get(selected).and_then(Option::as_ref).is_some()`: **call `freeze_on_user_mutation(state, persistence)` first** (which compacts panes; selected re-anchors to the same pane id at its new dense position), THEN remove `state.panes[selected]` (now `Some(p)` post-freeze), remove the matching entry from `persistence.bookmarks` (lookup via `pane_id_to_bookmark_idx[&p.id]`), decrement bookmark indices `index = Some(j)` where `j > selected`, remove the map entry for `p.id`, rebuild remaining map entries (their bookmark indices shifted down), clamp `selected` against new `panes.len()`. Return `[Effect::Save, Effect::Render]`. If selected target is `None` (placeholder) or out of range, return `vec![]` (no freeze)
    - `Char('j', plain)` / `Char('k', plain)`: nav with **wrapping** (matches today's existing fork behavior — `j` on bottom → `selected = 0`; `k` on top → `selected = panes.len() - 1`). Nav does NOT skip `None` (placeholder) entries — user can highlight a placeholder; mutation handlers will no-op on it. On empty list or single-element list, return `vec![]`. Otherwise return `[Effect::Render]`. No freeze (nav is not a mutation). (Asymmetry with `K`/`J` reorder is intentional: nav wraps because it's cheap to undo; reorder saturates because overshoot mutates persistent state.)
    - `Enter` / `Char('l', plain)`: **empty-list and placeholder guards** — if `state.panes.is_empty()` or `selected >= panes.len()` or `panes[selected].is_none()`, return `vec![]`. Else return `[Effect::Close, Effect::FocusPane(panes[selected].as_ref().unwrap().id)]` (in that order — close before focus). No freeze (focus is not a mutation)
    - `Char('c', _modifiers)`: **special-cased to accept ANY modifier set** including `Ctrl+c`. Return `[Effect::Close]`. No freeze. This preserves today's existing fork behavior where `BareKey::Char('c')` matches without modifier checking; users rely on `Ctrl+c` muscle memory to close the plugin
- [ ] 4.2 `/` (any modifier set, since `#`/`/` may arrive with implicit Shift on some keyboard layouts) → set mode to `Filter`, return `vec![Effect::Render]`
- [ ] 4.3 `#` (any modifier set, same reasoning as `/`) → set mode to `Jump`, return `vec![Effect::Render]`
- [ ] 4.4 `Esc` → return `vec![Effect::Close]`
- [ ] 4.5 **Digit-only** slot keys (`1-9`, modifiers plain): resolve via `state.panes.get(slot_index).and_then(Option::as_ref)`. If `Some(pane)` (Live), return `[Effect::Close, Effect::FocusPane(pane.id)]`. If `Some(None)` (placeholder — unresolved bookmark with that saved index), return `vec![]` (no-op; preserves saved-position contract during restore window). If `None` (slot beyond list length), return `vec![]`. Letter keys (`a-z`) MUST follow their existing command bindings or remain unbound; do NOT add letter slot jumps in command mode
- [ ] 4.6 `K` (Char('K') OR Char('k')+shift, modifier-set normalized) reorder up: pre-conditions — `panes.len() >= 2 && selected > 0 && selected < panes.len() && panes[selected].is_some() && panes[selected-1].is_some()` (both targets must be Live, no swapping with placeholders). If pre-conditions fail, return `vec![]` (no freeze, no Save, no Render). Else: **call `freeze_on_user_mutation(state, persistence)` first** (post-freeze, panes is dense and selected has re-anchored to the same pane id), THEN swap `panes[selected]` with `panes[selected-1]`, swap the corresponding bookmarks' `index` fields (both bookmarks are reachable via `pane_id_to_bookmark_idx`), decrement `selected`, return `[Effect::Save, Effect::Render]`. The Phase 0 probe (task 0.3) determines which form the host emits; the handler accepts BOTH forms defensively, since `harpoon-core::InputKey` normalization can collapse them
- [ ] 4.7 `J` reorder down: same modifier handling as K. Pre-conditions — `panes.len() >= 2 && selected < panes.len() - 1 && panes[selected].is_some() && panes[selected+1].is_some()`. If pre-conditions fail, return `vec![]`. Else: **freeze first**, swap, swap matching bookmarks' `index`, increment `selected`, return `[Effect::Save, Effect::Render]`
- [ ] 4.8 **Order-preservation refactor (P0 fix)**: implemented inside 4.1 (`a`/`A` branches). The legacy `sort_panes()` is NOT called from these paths. `A` iterates panes in `(tab.position ASC, PaneInfo.id ASC)` order so repeated runs produce the same result
- [ ] 4.9 Write failing unit tests FIRST:
  - K saturation at top emits `vec![]`; J saturation at bottom emits `vec![]`; K/J on empty list emit `vec![]`; K/J on 1-element list emit `vec![]`
  - **K/J on placeholder target emits `vec![]`** (e.g. selected on a Live pane but neighbor is a None placeholder — swap would move into the placeholder gap, which is rejected before freeze)
  - **K/J on Live targets triggers freeze**: setup a sparse `panes = [Some(P0), None, Some(P2)]` with bookmarks B0/B1(unresolved)/B2; press K with selected=2; assert post-handler state has dense `panes = [Some(P2), Some(P0)]`, B1 was rewritten to `index = None`, P2 is now bookmark 0, P0 is bookmark 1
  - digit `1` jump returns `[Close, FocusPane]` for Live target; placeholder slot `1` returns `vec![]`
  - letter `b` (no shift) returns `vec![]` in command mode (not bound, not a slot)
  - `d` on Live pane triggers freeze and emits `[Save, Render]`; `d` on placeholder selection emits `vec![]` (no freeze)
  - `a` add focused triggers freeze (when focused pane not yet pinned) and emits `[Save, Render, Close]`
  - `a` with focused pane already pinned emits `vec![]` (no freeze)
  - **`A` add-all emits `[Save, Render]` — NOT `[Save, Render, Close]`**; plugin remains open after `A`
  - `A` add-all is deterministic across runs (tab.position ASC, PaneInfo.id ASC)
  - Ctrl+`a` emits `vec![Effect::Noop]` (modifier-gated)
  - **Ctrl+`c` emits `vec![Effect::Close]` — carve-out from modifier gating**; plain `c` also emits `vec![Effect::Close]`
  - Other modified letters (Ctrl+`d`, Alt+`a`, Super+`k`) emit `vec![Effect::Noop]`
  - **`j` from bottom of list wraps to top** (`selected = 0`, returns `[Render]`); `k` from top wraps to bottom (`selected = panes.len() - 1`, returns `[Render]`)
  - **`j`/`k` may land on a `None` (placeholder) row** — nav still emits `[Render]`, no freeze; subsequent `Enter`/`l`/`d` while selected on placeholder emits `vec![]`

## 5. Filter mode

- [ ] 5.1 Implement `handle_filter_key(state, ctx, key)`. Printable char arms gate on `modifiers.is_plain()` (post-FFI normalization, so Shift+letter has already been collapsed to uppercase + empty modifier set; only the empty modifier set passes the gate). Modified variants (Ctrl/Alt/Super + char) return `vec![Effect::Noop]` so `Ctrl+W`/`Ctrl+C`/`Alt+a` are not consumed as query input. Backspace pops; return `vec![Effect::Render]`. Backspace on empty query returns `vec![]`
- [ ] 5.2 On every query mutation (non-empty query): set `selected = 0`. The shim re-runs `filtered_indices` on next dispatch via the matcher pass; `clamp_selected_to_view(state, ctx.filtered_indices.len())` is called by the shim post-handler. On query becoming empty: call `reanchor_selected_to_focus(state, ctx.focused_pane.as_ref().and_then(|p| panes_index_of(p)))` to re-anchor to focused pane (with fallback to `0` when focused pane is None or not in `panes`). Handler reads `ctx.focused_pane`; helper functions own the index resolution
- [ ] 5.3 Arrow keys (`Up`/`Down`) navigate within filtered view (`ctx.filtered_indices`), clamped to `[0, ctx.filtered_indices.len().saturating_sub(1)]`; return `vec![Effect::Render]` if `selected` actually changed, else `vec![]`
- [ ] 5.4 `Enter`: if `ctx.filtered_indices.is_empty()` return `vec![]`; else resolve to the underlying pane via `state.panes[ctx.filtered_indices[selected]].as_ref()`. Filtered indices reference Live panes only (placeholders excluded from filter view), so `as_ref()` yields `Some(p)` always. Return `[Effect::Close, Effect::FocusPane(p.id)]`
- [ ] 5.5 `Esc` non-empty query → clear query, call `reanchor_selected_to_focus(state, ctx.focused_pane_idx())` (matches backspace-to-empty behavior), return `vec![Effect::Render]`. `Esc` empty query → set mode to `Command`, return `vec![Effect::Render]`
- [ ] 5.6 Implement `filtered_indices(state: &DispatchState, matcher: &mut MatcherImpl) -> Vec<usize>`. **Empty-query short-circuit**: if `state.query.is_empty()`, return all indices `i` where `state.panes[i].is_some()` (Live panes only; placeholders excluded). Otherwise iterate Live panes, run the matcher against each, and return their `panes` indices ordered by `(score DESC, panes_index ASC)` — placeholders are NEVER included in the filtered view; the matcher only sees Live haystacks
- [ ] 5.7 In `update_panes()` (plugin crate), call `reanchor_selected_to_focus(&mut self.dispatch_state, focused_idx)` instead of inline focused-pane → `selected` write. After every pane mutation in `update_panes`, also call `clamp_selected_to_view(&mut self.dispatch_state, current_view_len)` where `current_view_len = if filter_mode_with_query { last_filtered_indices.len() } else { panes.len() }`
- [ ] 5.8 Write failing tests FIRST: pane update during non-empty filter does not move `selected`; filtered view shrink clamps `selected`; query clear with focused pane re-anchors to focused index; query clear with `focused_pane = None` falls back to `selected = 0`; backspace on empty query returns `vec![]`; equal-score matches break tie by original index; `Enter` on empty filtered view returns `vec![]`; `Ctrl+W` in filter mode returns `vec![Effect::Noop]` (does NOT append `w` to query)

## 6. Jump mode

- [ ] 6.1 Implement `handle_jump_key(state, ctx, key)`: `1-9`/`a-z` (modifier-plain post-normalization) → resolve via `state.panes.get(slot_index).and_then(Option::as_ref)`. If `Some(pane)` (Live), return `[Effect::Close, Effect::FocusPane(pane.id)]`. If `Some(None)` (placeholder), return `vec![]` (no-op; saved-position contract preserved). If `None` (beyond list), return `vec![]`. No freeze (jump is not a mutation)
- [ ] 6.2 `Esc` → set mode to `Command`, return `vec![Effect::Render]`
- [ ] 6.3 All other keys (uppercase letters, Backspace, arrows, etc.) → return `vec![]` (no Render, no mutation)
- [ ] 6.4 In `harpoon-core`: `pub fn slot_index_from_char(c: char) -> Option<usize>`: `'1'..='9'` → `0..=8`, `'a'..='z'` → `9..=34`, else `None`
- [ ] 6.5 In `harpoon-core`: `pub fn slot_char_from_index(i: usize) -> Option<char>`: inverse of above
- [ ] 6.6 Write failing tests FIRST: digit slot 1 → index 0; letter slot `b` → index 10; `z` → index 34; out-of-range slot returns `None`; uppercase letters return `None`; round-trip `slot_char_from_index(slot_index_from_char(c).unwrap())` returns `c` for all valid inputs
- [ ] 6.7 Write failing tests for placeholder no-op: in jump mode with a placeholder bookmark at saved index 1 and no live pane there, pressing `2` returns `vec![]` (no Close, no FocusPane). When the underlying bookmark resolves and `state.panes` now has a live pane at index 1, pressing `2` returns `[Effect::Close, Effect::FocusPane(id)]`

## 7. Render layer (plugin crate only)

## 7. Render layer (builders in core, `Text` translation in plugin)

Layout/string-building lives in `harpoon-core` as pure functions returning `RenderRow` / `RenderHeader` descriptors; the plugin shim translates descriptors to `zellij_tile::Text`. This keeps layout decisions natively-testable.

- [ ] 7.1 In `harpoon-core`, define:
  ```rust
  pub struct RenderRow { pub text: String, pub highlight_indices: Vec<usize>, pub highlight_kind: HighlightKind, pub is_selected: bool, pub is_placeholder: bool }
  pub struct HeaderLine { pub text: String, pub badge_range: Option<Range<usize>>, pub query_range: Option<Range<usize>> }
  pub struct RenderHeader { pub lines: Vec<HeaderLine>, pub height: u16 }
  pub enum HighlightKind { None, FuzzyChars, SubstringRange { start: usize, end: usize } }
  ```
  Per-line metadata so a 2-line narrow-width header can carry distinct badge/query ranges per line. **No separate `RowEntry` enum needed** — the sparse `Vec<Option<Pane>>` IS the row source: `Some(p)` renders as live, `None` renders as a placeholder. Placeholder text is derived from the matching bookmark via `persistence.bookmarks` (looked up by `index = Some(panes_idx)`)
- [ ] 7.2 In `harpoon-core`, `pub fn build_header(state: &DispatchState, cols: usize, max_height: u16) -> RenderHeader`: emits standard `==== N panes ====` for command/jump or `/<query> (<m>/<n>)` for filter with non-empty query; appends `[N]/[F]/[J]` badge; returns `Vec<HeaderLine>` + consumed height (≤ max_height). Implements narrow-width truncation order (drop count first; then if max_height ≥ 2, badge moves to its own line; then truncate query with leading ellipsis). At max_height == 1, badge stays inline and query is truncated more aggressively
- [ ] 7.3 In `harpoon-core`, `pub fn build_rows(state: &DispatchState, placeholder_lookup: &dyn Fn(usize) -> Option<(String, String)>, matcher: &mut MatcherImpl, max_rows: u16) -> Vec<RenderRow>`: walks `state.panes` directly. For each `i in 0..panes.len()`:
  - `Some(p)` → build a Live `RenderRow` (slot prefix + display string, highlights for filter mode if matched)
  - `None` → call `placeholder_lookup(i)` to get the saved `(tab_name, pane_title)` for the placeholder at this slot; build a placeholder `RenderRow` with text `"<slot>  ?  (resolving)"` (or `"   ?  (resolving)"` when `show_slots = false`); set `is_placeholder = true`
  
  In **Filter mode** with non-empty query, build_rows iterates `ctx.filtered_indices` instead, which contains only Live indices (placeholders excluded from filter view). Suppresses slot prefix; populates `highlight_indices` / `highlight_kind` per matcher result.
  
  Plugin shim builds the `placeholder_lookup` closure by walking `persistence.bookmarks` once into a `HashMap<usize, (String, String)>` keyed on `index.unwrap_or(usize::MAX)`, restricted to bookmarks not present in `pane_id_to_bookmark_idx` (i.e. unresolved). The closure looks up by `panes_idx`.
- [ ] 7.4 In `harpoon-core`, `pub fn build_hint_line(mode: Mode, cols: usize) -> String`: mode-aware. Budget at column widths 80, 50, 30 for each mode. Command at ≥80: `a/A add  d del  K/J reorder  1-9 jump  / filter  # jump  Esc close`; at ≥50: drop labels, abbreviate; at ≥30: minimal. Filter at ≥80: `Esc clear/exit  Enter focus  ↑/↓ nav`; progressively shorter. Jump at ≥80: `1-9/a-z jump  Esc back`; progressively shorter
- [ ] 7.5 Plugin: render `RenderHeader.lines` starting at `HEADER_BASE_Y` (constant from Phase 0 task 0.1). For each `HeaderLine` with `badge_range: Some(r)`, apply `Text::color_range(level=2, r)` for badge accent **regardless of mode** (badge color renders in command, filter, AND jump per spec). If `query_range: Some(_)`, query renders as plain text (no extra color). Render rows starting at `HEADER_BASE_Y + header.height`; for each `RenderRow`: build `Text::new(&row.text)`, then dispatch by `highlight_kind`: `FuzzyChars` → `Text::color_indices(level=1, row.highlight_indices.clone())`; `SubstringRange { start, end }` → `Text::color_range(level=1, start..end)`; `None` → no color; if `is_selected` apply `.selected()`. **Placeholder rows** (where `is_placeholder = true`) render dimmed — use a separate `Text` color level (e.g. existing dim/comment color level if `Text` exposes one, else just leave plain text — the `?  (resolving)` content is its own visual cue). **Color levels: 1=highlight, 2=badge, 3=hints (existing)**
- [ ] 7.5b **Audit existing `Text::color_*` calls in `src/main.rs`** before pinning levels: confirm the existing `build_hint_line` actually uses level 3 (or whichever level), and update the level mapping in design.md if there's a collision
- [ ] 7.6 Plugin: layout precedence at small heights. Compute `max_header_height = if rows >= 4 { 2 } else { 1 }` and pass to `build_header`. Hint line renders at `y = rows.saturating_sub(1)` only when `rows.saturating_sub(header.height) >= 2` (need at least 1 row of panes plus the hint); otherwise hint is dropped. Row loop bound is `rows.saturating_sub(if hint_visible { 2 } else { 1 }).saturating_sub(header.height)`. Add a unit test in `harpoon-core` for hint visibility / row budget at heights 2, 3, 4, 6, 24
- [ ] 7.7 Write failing tests FIRST in `harpoon-core` (native target): header truncation at cols 80/40/20 produces expected `RenderHeader.height` and `lines.len()`; hint builder at 80/50/30 cols stays ≤ cols for each mode; `build_rows` for filter mode with multi-byte haystack `"📦 build"` and substring needle `"b"` produces `highlight_kind: SubstringRange { start: 2, end: 3 }`; `build_rows` in command mode prefixes rows with slot characters and suppresses prefix in filter mode; per-mode `build_header` always populates `badge_range: Some(_)` on at least one line; **`build_rows` with `panes = [Some(P), None, Some(P2)]` and a `placeholder_lookup` returning `Some((tab, title))` for index 1 produces 3 rows: live, placeholder (`"2  ?  (resolving)"` with `is_placeholder = true`), live**; **`build_rows` in filter mode skips `None` slots entirely** (filtered_indices contains only Live indices); **`build_rows` placeholder text matches `"<slot>  ?  (resolving)"` format with correct slot character**
- [ ] 7.8 **If Phase 0.5 fails (host indexes by bytes)**: this task is the deferred fallback. Add `pub fn char_indices_to_bytes(s: &str, char_indices: &[usize]) -> Vec<usize>` to `harpoon-core`; render shim converts `RenderRow.highlight_indices` from char to byte before passing to `Text::color_*`. Update `specs/filter-mode/spec.md` "Match highlighting via character indices" requirement title and scenarios to assert byte indices. Update unit tests for the conversion helper (multi-byte haystacks)

## 8. Persistence schema upgrade & restore buffer

- [ ] 8.1 In `harpoon-plugin/src/persistence.rs`, add `index: Option<u16>` field to `PaneBookmark` (with `#[serde(default)]` so missing field deserializes to `None`), define the v2 envelope wrapping a single bookmarks Vec, and add the pane-id↔bookmark identity map as a first-class field on `Persistence`:
  ```rust
  #[derive(Serialize, Deserialize)]
  struct PersistedV2 { version: u8, bookmarks: Vec<PaneBookmark> }
  
  pub struct Persistence {
      pub bookmarks: Vec<PaneBookmark>,                    // canonical disk shape
      pub pane_id_to_bookmark_idx: HashMap<u32, usize>,    // identity map; not persisted
      last_saved_state: Option<PersistedV2>,
      // ... existing fields (file path resolver, etc.)
  }
  ```
  The map is **not persisted** — it's rebuilt on load (empty initially) and populated as bookmarks resolve via `update_panes`. It's the authoritative answer to "which bookmark corresponds to this live pane?"
- [ ] 8.2 `save_to_disk()` (no-arg form) writes the current `Persistence::bookmarks` Vec directly: `serde_json::to_writer(file, &PersistedV2 { version: 2, bookmarks: self.bookmarks.clone() })`. **No rebuild step from `state.panes`** — `Persistence::bookmarks` is the canonical disk shape and is kept in sync at every mutation by the dispatch handlers (see tasks 4.1, 8.4). After save, `last_saved_state = Some(persisted_v2)` for `has_changed` comparison.

  **Critical**: this means unresolved bookmarks with `index = Some(_)` (saved-index restore awaiting late resolution) are written to disk faithfully even during partial restore. The earlier rebuild-from-panes design would have truncated them — P0 regression closed by this change
- [ ] 8.3 Load path: try to deserialize as `PersistedV2` first (v2 envelope). If that fails, fall back to `Vec<PaneBookmark>` (v1 bare array). On v1 success: assign `index = Some(i as u16)` in array order. (No explicit "mark for migration" needed — every save writes v2 envelope, so the next save naturally writes v2.) On both fail: log `LoadFromDiskFailed` and start with empty bookmarks
- [ ] 8.4 **Sparse-Vec restore model**: `state.panes: Vec<Option<Pane>>` and `Persistence::bookmarks: Vec<PaneBookmark>` carrying `index: Option<u16>`. Resolution semantics on each `update_panes` round:
    - First pass: ensure `state.panes` is grown to accommodate saved indices. Compute `max_saved_idx = persistence.bookmarks.iter().filter_map(|b| b.index).max().unwrap_or(0)`. If `state.panes.len() < max_saved_idx + 1`, grow with `state.panes.resize(max_saved_idx + 1, None)`. (Only happens during initial restore window; once dense, max_saved_idx ≤ state.panes.len() always holds.)
    - For each unresolved `bookmark` (i.e. no entry in `pane_id_to_bookmark_idx` maps to it yet) whose `(tab_name, pane_title)` matches a visible pane in `ctx.visible_panes` (first match wins, consume per-round to distribute across duplicates):
      - If `bookmark.index = Some(i)`: set `state.panes[i] = Some(pane)`. Insert `pane.id → bookmark_idx` into `pane_id_to_bookmark_idx`.
      - If `bookmark.index = None`: push `Some(pane)` to `state.panes`; rewrite the bookmark's `index = Some(state.panes.len() - 1)` (so the bookmark's saved index now reflects its actual position); insert map entry.
    - Bookmarks not yet visible remain in `Persistence::bookmarks` unchanged. Their slot indices render as placeholders via `build_rows` walking `state.panes` (encountering `None` at those positions).
  
  **Freeze on user mutation**: see task 4.0 — freeze is a separate helper invoked as a pre-step from each mutation handler that determined it WILL mutate. Resolution and freeze are decoupled: resolution happens on every `update_panes`; freeze happens only on first user mutation of the session.
  
  **Save semantics**: `save_to_disk()` (no arg) writes `self.bookmarks.clone()` directly. No rebuild. The `Persistence::bookmarks` Vec is the authoritative shape, kept in sync inline by add/delete/reorder/freeze/restore-resolution.
  
  **Pane disappearance**: when `update_panes` filters via the existing `get_valid_panes` (drops panes whose id is no longer in `PaneManifest`), pre-freeze (sparse window) and post-freeze (dense) handle disappearance differently:
  
  - **Pre-freeze (sparse window, `state.panes` may contain `None` slots)**: for each `Some(pane)` whose id is no longer in `valid_ids`, set `state.panes[i] = None` (revert to placeholder) AND remove `pane.id` from `pane_id_to_bookmark_idx`. The bookmark itself stays in `Persistence::bookmarks` unchanged. This way, if the same `(tab_name, pane_title)` is observed again later, restore resolution can re-claim the slot. Placeholder `None` slots are preserved untouched.
  
  - **Post-freeze (dense, no `None` slots)**: for each `Some(pane)` whose id is no longer in `valid_ids`, remove the entry entirely (`state.panes.retain(|opt| valid_ids.contains(&opt.as_ref().unwrap().id))`) AND remove the matching bookmark from `Persistence::bookmarks` AND remove the map entry. This matches today's existing fork behavior — a closed pane is forgotten. Bookmark indices for surviving entries are recomputed.
  
  The pre/post-freeze distinction is detected by a `Persistence::is_frozen() -> bool` flag set by `freeze_on_user_mutation` (false initially, set to true on first freeze, never reset within a session). This is the "consume-on-first-resolve" semantics for the post-freeze world, while preserving the saved-position contract pre-freeze.
- [ ] 8.5 `K`/`J` reorders trigger `Effect::Save` (gated on `has_changed` in 3.5; full-envelope comparison from 8.6)
- [ ] 8.6 **`Persistence::has_changed(&self) -> bool`** (no-arg form): compares the current `self.bookmarks` against `last_saved_state.as_ref().map(|s| &s.bookmarks)`. Returns true when they differ. Compare via `PartialEq` on `Vec<PaneBookmark>` (cheapest) or bytewise via `serde_json::to_vec(&PersistedV2 { version: 2, bookmarks: self.bookmarks.clone() })` if `PartialEq` derive contention arises
- [ ] 8.6b `Persistence::save_if_changed(&mut self) -> Result<(), Error>` (no-arg form): calls `self.has_changed()`; if true, calls `self.save_to_disk()` and updates `last_saved_state`. Single canonical save entry point used by both `Effect::Save` (key dispatch) and the existing `update_panes` tail call (non-Key events). Confirm tasks 3.5 and 3.5b call `self.persistence.save_if_changed()` with no arguments — the canonical shape lives on `Persistence::bookmarks`, which the dispatch handlers keep in sync as they mutate `state.panes`
- [ ] 8.7 **Remove `sort_panes()` from non-initial paths (P0 fix)**: the `a`/`A` paths must NOT re-sort (handled in 4.1 — each push appends to the end of `state.panes` post-freeze). The restore resolution path uses saved-index placement (8.4), NOT re-sort. If `sort_panes()` is retained at all, it MUST run only on the very first population of an empty `state.panes` with no persisted state
- [ ] 8.8 Unit tests (in `harpoon-core` if restore logic extracted, else `harpoon-plugin/tests`):
  - **Single-round restore preserves saved order**: 3 bookmarks at indices 0/1/2, all visible in round 1 → `state.panes = [Some(P0), Some(P1), Some(P2)]`
  - **Staggered restore preserves saved positions**: bookmarks B0(idx=0)/B1(idx=1)/B2(idx=2). Round 1 visible = {B0, B2}; after round: `state.panes = [Some(P0), None, Some(P2)]`. Round 2 visible adds B1; after round: `state.panes = [Some(P0), Some(P1), Some(P2)]`. Slot key `2` during round 1 returns `vec![]` (placeholder); after round 2, slot `2` returns `[Close, FocusPane(P1.id)]`
  - **User mutation during partial restore freezes**: after the round-1 sparse state above, press K with selected=2. Pre-conditions fail (selected-1 is None placeholder), so K returns `vec![]` with NO freeze. Move selected to 0, press d. Pre-conditions pass (panes[0] is Some(P0)); freeze fires; post-freeze `state.panes = [Some(P0), Some(P2)]` (B1 was unresolved Some → None); then d removes panes[0], result: `state.panes = [Some(P2)]`, bookmarks: [B1(None), B2(idx=0)]
  - **`has_changed` detects duplicate-title swap**: 2 bookmarks both `(work, nvim)` at indices 0/1; identity tracked by pane id via the map; K swap updates indices to (0→1, 1→0); `has_changed()` returns true comparing canonical bookmarks Vec
  - **`pane_id_to_bookmark_idx` invariant**: for every entry in `state.panes` that is `Some(p)`, `pane_id_to_bookmark_idx[&p.id]` exists and points to a bookmark in `persistence.bookmarks`. Run after every test scenario as a sanity check
- [ ] 8.9 Manual smoke: reorder, close zellij, reopen, verify slot mapping is preserved
- [ ] 8.10 Manual smoke: reorder, then add a new pane via `a`, verify the new pane appends at the end and existing order is preserved
- [ ] 8.11 Manual smoke: open a session with two duplicate-titled panes in the same tab, reorder them with K/J, verify the swap persists (in-session). Reload and verify the documented best-effort restore behavior

## 9. Validation

- [ ] 9.1 `cargo build -p harpoon --target wasm32-wasip1 --release` from workspace root (or `cd harpoon-plugin && cargo build --release`) succeeds with no warnings. Note: package name is `harpoon`, NOT `harpoon-plugin` (the latter is the directory name only). The target flag is required from workspace root because cargo does NOT inherit `harpoon-plugin/.cargo/config.toml` when invoked from outside that directory
- [ ] 9.2 `cargo test -p harpoon-core` passes (matcher, dispatch, mode, slot, config, reconcile_selected, restore-ordering tests)
- [ ] 9.3 Manual scenario walk-through using `plugin-dev-workspace.kdl` for each spec scenario in `specs/mode-state-machine`
- [ ] 9.4 Manual walk-through for `specs/filter-mode` scenarios (typing, Backspace, Enter, Esc clear, fuzzy highlight via color_indices, substring highlight via color_range, multi-byte haystack alignment, tie-breaker stability)
- [ ] 9.5 Manual walk-through for `specs/jump-mode` scenarios (digit slots from command, digit + letter slots from jump, no letter jumps from command, empty slot no-op, slot prefix suppressed in filter)
- [ ] 9.6 Manual walk-through for `specs/reorder` scenarios (K/J shifts, saturate, slot remap, persist, single-round + staggered reload preserves order, add-after-reorder preserves order, duplicate-title best-effort)
- [ ] 9.7 Manual walk-through for `specs/plugin-config` scenarios (each config key default, valid, garbage, mixed-case)
- [ ] 9.8 Verify badge + color visible in each mode at narrow/medium/wide pane widths; verify narrow-width truncation order produces the expected layout

## 10. Documentation

- [ ] 10.1 Update `README.md` with the new mode model (state diagram), per-mode keymap tables, and Esc semantics; explicitly document that letter slots require entering jump mode
- [ ] 10.2 Document `default_mode`, `matcher`, `show_slots` config keys with example kdl block; note case-insensitive parsing
- [ ] 10.3 Add a "Reordering" section showing K/J and slot-mapping behavior, including the order-canonical guarantee across session reloads and the duplicate-title limitation
- [ ] 10.4 Note the upstream divergence — this fork no longer matches `Nacho114/harpoon`'s key model
- [ ] 10.5 Document the persistence schema v1 → v2 migration (transparent to users; v1 files auto-upgrade on next save)
- [ ] 10.6 Note the carried-forward known macOS focus bug in the README "Known issues" section

## 11. Release

- [ ] 11.1 Self-review the diff against the spec scenarios — every scenario maps to observable behavior or to a unit test
- [ ] 11.2 Squash or curate commits to keep fork history readable (custom: prefix new commits)
- [ ] 11.3 Push to `origin/main` (personal fork)
- [ ] 11.4 Build release wasm and copy into `~/.config/zellij` plugin directory; sanity check live
