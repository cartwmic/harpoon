## Context

`harpoon-zellij` is a Rust/wasm Zellij plugin (forked from `Nacho114/harpoon`) that lets the user pin terminal panes for fast jumping. Today every printable key in the plugin is a single-purpose command (`a` add, `d` delete, `j`/`k` nav, `Enter` focus, `Esc` close). The personal fork already carries several custom commits (sticky `focused_pane`, per-session persistence, hint-line UI), and this proposal extends the fork further — there is no intent to upstream the change.

The current state has three structural constraints we must respect:

1. **Plugin lifecycle**: The plugin instance is *not* destroyed on `hide_self()`. State persists across opens, so any "reset on close" behavior must be implemented explicitly.
2. **`focused_pane` capture**: The existing fork commit `da678cb` (sticky `focused_pane`) captures the user's previous pane on every `update_panes`, only overwriting when a real terminal pane is focused. The new mode model must not regress this.
3. **Persistence and ordering**: The persistence schema is being upgraded from v1 (bare `Vec<PaneBookmark>` JSON array, no `index` field) to v2 (envelope `{version: 2, bookmarks: [...]}` where each `PaneBookmark` carries `index: Option<u16>`). The schema change is required to preserve manual reorder across staggered (multi-round) restore: without a saved index, late-resolving bookmarks would be appended rather than placed at their saved position. Additionally, the existing `sort_panes()` helper re-sorts `State.panes` by `tab_info.position` on every add (`a`/`A`) and on every `match_pending_bookmarks` resolution. That sort is incompatible with user-driven reorder: it would silently undo `K`/`J` operations on the next add or session reload. This change reclassifies manual order as authoritative and removes/restricts those re-sort calls — see the "Manual order is canonical" decision below.

Stakeholder is a single user (`cartwmic`); breaking changes are acceptable, but the default config should preserve today's bare-key UX so existing muscle memory still works.

## Goals / Non-Goals

**Goals:**
- Three first-class, mutually-exclusive interaction modes (filter / command / jump) with a configurable default.
- Vim-style Esc transitions: filter ↔ command, jump → command, command → close.
- Type-to-filter with fuzzy matching and live match-highlighting.
- Numeric/letter slot hotkeys (1-9, a-z) jumping the user to the pane at that slot.
- A reorder operation in command mode (`K`/`J`) that mutates the pane Vec and therefore the slot mapping.
- Plugin config (`default_mode`, `matcher`, `show_slots`) read from `ZellijPlugin::load`'s `BTreeMap`.
- Match highlighting renders correctly via `Text::color_range`, with indices offset past the slot prefix.
- All `hide_self` paths consolidated through a `close()` helper that resets `mode` and `query`.

**Non-Goals:**
- Multi-digit jump shortcuts (slots beyond 35 stay reachable only via filter/nav).
- Substring or regex filtering modes beyond the two named (`fuzzy`, `substring`).
- Customizable keybindings — the keymap is fixed; only the *default mode* is configurable.
- Telescope-style two-pane preview, sorting algorithms beyond fuzzy score, or per-row metadata badges.
- Upstream PR. This is fork-only.
- A `swap-to-slot` (e.g. `m`-then-digit) reorder operation. `K`/`J` is sufficient.
- Custom theming or user-configurable colors.

## Decisions

### Decision: Three explicit modes vs. unified modeless command surface

**Choice**: Three explicit, mutually-exclusive modes (`Filter`, `Command`, `Jump`).

**Rationale**: Filter mode needs printable chars to feed a query buffer, which is fundamentally incompatible with today's "every char is a command" model. A single-mode design with modifier-keyed commands (Alt-a, Alt-d, …) was considered but rejected because (a) Alt-h/j/k/l collides with zellij's default pane-nav muscle memory, and (b) the user explicitly preferred a vim-style modal model.

**Alternatives considered**:
- Modifier-only commands (Scheme 1 from explore): simpler state machine but heavier finger load and key-collision risk.
- Hybrid (modifiers always work + optional submode): more code without proportional UX wins.

### Decision: Vim-style Esc semantics

**Choice**: Filter mode with non-empty query → Esc clears query (stays in filter). Filter with empty query → Esc returns to command. Jump → Esc returns to command. Command → Esc closes.

**Rationale**: User picked this directly. Mirrors `:` / normal / insert vim transitions. The cost is up to three Escs to close from a typed query in filter mode, which is acceptable because (a) typed-query close is rare and (b) `c` from command remains a one-key close.

### Decision: Slot mapping uses `1-9` then `a-z` for 35 jumpable slots

**Choice**: `panes[0]` → `1`, `panes[8]` → `9`, `panes[9]` → `a`, `panes[34]` → `z`. `panes[35..]` has no slot shortcut.

**Rationale**: Mirrors Neovim harpoon's spirit (a small fixed addressable set). Avoids multi-digit input with timeouts. 35 slots is far more than the user is expected to keep pinned. Beyond 35, panes are still reachable via filter or arrow nav.

**Alternatives considered**:
- Cap at 9 (option D from explore): simpler but tight.
- Multi-digit with timeout: annoying input mode.

### Decision: Slot prefixes visible in command and jump modes only

**Choice**: Render `1  `, `2  `, ..., `9  `, `a  `, ... as a 3-char prefix on every row in `Command` and `Jump` modes when `show_slots = true` (default). In `Filter` mode the prefix is suppressed.

**Rationale**: Discoverability and muscle-memory consistency in command/jump mode. Suppressing in filter mode is required because (a) filtered rows are reordered by match score and a slot prefix would no longer correspond to row position, and (b) pressing the prefix character in filter mode appends to the query rather than firing a jump, which would be a UX trap. See `specs/jump-mode/spec.md` "Slot prefix rendering in command and jump modes".

### Decision: Digit-only slot jumps in command mode

**Choice**: In `Command` mode, ONLY digit slot keys (`1-9`) trigger a jump-and-close. Letter slot keys (`a-z`) retain their existing command-mode bindings (`a` add, `c` close, `d` delete, `j`/`k` nav, `l` focus). To use letter slots, the user must first enter `Jump` mode via `#`.

**Rationale**: Letters `a`, `c`, `d`, `j`, `k`, `l` are already bound to commands in `Command` mode. Allowing them to also fire jumps would either break existing bindings (worse muscle-memory regression) or create ambiguity that has to be resolved by precedence — neither is a good trade. Digits `1-9` are unbound in command mode today, so digit jumps are strictly additive. `Jump` mode remains the only way to address the full 35-slot range, which preserves the read-only safety property of jump mode for letter inputs and gives users one clean mental model: "digits jump from anywhere; letters require jump mode."

**Alternatives considered**:
- Full `1-9`+`a-z` jumps from command mode (original proposal): rejected as unimplementable without breaking existing letter commands.
- Drop the conflicting letter commands (`a`/`d`/`j`/`k`/`l`/`c`) entirely and re-bind to non-letter keys: too disruptive to existing muscle memory.
- Slot prefix character chosen at runtime to avoid command keys: too magical, harms discoverability.

### Decision: Manual reorder is canonical post-first-add

**Choice**: Once `State.panes` has any entries, code paths that previously re-sorted by `tab_info.position` are modified or removed. Specifically:

- The `a` and `A` add paths append new panes to the end of `State.panes` without re-sorting existing entries.
- `match_pending_bookmarks` resolution restores panes into their saved positions and does not subsequently re-sort.
- The legacy `sort_panes()` helper is either removed or downgraded to a one-time initial sort on a truly empty `State.panes` with no persisted state.

**Rationale**: The user-facing `K`/`J` reorder operation is meaningless if any subsequent add or session reload silently re-sorts. Persistence of the on-disk JSON format alone is not enough; the load and add pipelines must respect the saved order rather than recomputing it.

**Alternatives considered**:
- Store an `order` field on `Pane` and sort by that: redundant; the Vec position already encodes order if we stop fighting it.
- Keep `sort_panes()` and add a "user reordered" flag that disables it: more state, easier to forget to set.

### Decision: Filter `selected` semantics — index into the filtered view, reset to 0 on query change

**Choice**: While filter mode is active, `selected` is interpreted as an index into the recomputed `filtered_indices` slice. Empty query → behaves as an alias of the full pane list (selected starts on focused pane, today's behavior). Any non-empty query change → `selected = 0` (top match).

Additionally, `update_panes()`'s focused-pane → `selected` reconciliation is gated on `mode == Command || (mode == Filter && query.is_empty())`. With a non-empty query, pane/tab update events MUST NOT overwrite `selected`. After any pane mutation (add, delete, reorder, restore), `selected` is clamped against the current view length so it never exceeds `view.len() - 1`.

**Rationale**: Matches fzf/Telescope semantics. Trying to "track the previously selected pane through query changes" looks clever but feels jittery — every keystroke can reorder matches, so anchoring to the top is more predictable. The `update_panes` gate is required because zellij events arrive at unpredictable times, including mid-keystroke; without the gate, a `PaneUpdate` could silently snap `selected` to the focused-pane index in the FULL list while the user is navigating the FILTERED list, producing wrong-target focus when the user presses Enter.

### Decision: Matcher dependency — try `nucleo-matcher` first, fall back in-tree

**Choice**: Add `nucleo-matcher` (pure Rust, used by Helix/Zellij itself) as a direct dep. If it fails to build for `wasm32-wasip1`, replace with a ~50-line in-tree subsequence matcher with word-boundary bonuses.

**Rationale**: `nucleo-matcher` exposes match-index extraction (needed for highlighting) and is well-tested. The risk is wasm target compatibility; the fallback is cheap. A spike (Phase 0 in `tasks.md`) decides this empirically before the rest of the work.

**Alternatives considered**:
- `fuzzy-matcher` (skim): also pure Rust, slightly older API.
- Pure in-tree: zero deps but loses years of tuning.

### Decision: Reorder via `K`/`J` only, no `m`-then-digit

**Choice**: `K` (shift-k) moves selected pane up by one. `J` (shift-j) moves down by one. Both saturate at boundaries (no wrap). `selected` follows the moved pane.

**Rationale**: Reordering is rare and small adjustments dominate. `m`-then-digit was considered but rejected for simplicity — can be added later without breaking anything.

### Decision: Mode reset on close, not on hide

**Choice**: A new internal `close()` helper consolidates all `hide_self()` callsites. It calls `hide_self()`, then resets `mode = self.default_mode` and `query.clear()`. `selected` is not explicitly reset because the next `update_panes()` cycle re-anchors it to `focused_pane`.

**Rationale**: The plugin instance survives `hide_self`, so without an explicit reset the next open inherits stale mode/query state. Centralizing through `close()` removes the risk of forgetting a reset on a new keybind.

### Decision: Mode badge `[N]`/`[F]`/`[J]` plus per-mode color accent

**Choice**: A 3-char bracketed badge in the header line, plus a `Text::color_range` accent on the header for mode-distinct color (blue=command, yellow=filter, green=jump or similar). Badge uses `[N]` for command (vim normal) for visual symmetry.

**Rationale**: Badge alone is colorblind-safe; color alone is more glanceable. Both is the strict superset. The 3-char badge fits in the existing header line without layout work.

**Narrow-width truncation order**: At column widths where the filter header `"/<query>  (<m>/<n>)  [F]"` does not fit on one line, the render layer drops elements in this priority order: (1) the count `(m/n)` is dropped first; (2) if still too wide, the badge moves to a separate row above the query; (3) if still too wide, the query is truncated with a leading ellipsis (`/…last_chars`) so the most recently typed characters remain visible. The badge is never dropped entirely.

### Decision: Match-highlight indices are character indices, applied via `color_indices` for fuzzy and `color_range` for substring

**Choice**: The `Matcher` trait returns `(score, Vec<usize>)` where the `Vec<usize>` contains **character (Unicode scalar) indices** into the haystack, not byte offsets.

The render layer dispatches on the matcher type:
- For fuzzy matches (potentially non-contiguous indices): `Text::color_indices(level, indices.clone())`. `zellij-tile`'s `color_indices` is the API designed for arbitrary index lists; it lives next to `color_range` in `ui_components/text.rs`.
- For substring matches (always contiguous): `Text::color_range(level, start..end)` where `start` and `end` are char indices spanning the substring match.

**Rationale**: `Text::color_range` accepts a single `RangeBounds<usize>` and is suitable only for contiguous spans — fuzzy matches are non-contiguous by definition (subsequence of chars). Using `color_range` for fuzzy would either fail to compile against a `Vec<usize>` or, if called once per char, abuse the API. `color_indices` takes `Vec<usize>` directly and matches the fuzzy contract.

Both APIs treat their numeric arguments as character (Unicode scalar) positions — verified in `zellij-tile-0.42.2/src/ui_components/text.rs` (`color_range`'s unbounded-end branch evaluates `text.chars().count()`). Returning character indices from the matcher avoids a byte↔char conversion at the render site.

For `nucleo-matcher` integration, the matcher's reported indices (which are char positions when fed via the appropriate `Utf32Str` / `Utf32String` constructor) are consumed directly. The Phase 0 spike asserts char-index semantics with a multi-byte test haystack (`"📦 build"`, needle `"b"` → expected index `2`, not byte offset `4`). For an in-tree fallback subsequence matcher, the implementation iterates the haystack with `char_indices()` and records the **char position** (not the byte index) of each matched character.

### Decision: Pure key-dispatch core for testability

**Choice**: The per-mode key handlers are refactored into pure functions of the form `fn handle_<mode>_key(state: &mut State, key: BareKey) -> Vec<Effect>` where `Effect` is an enum of `{ Render, Close, FocusPane(u32), Save, Noop }`. The `update()` method becomes a thin shim that dispatches by mode, collects effects, and applies side effects in a single block.

**Effect application order**: Effects in the returned `Vec<Effect>` are applied in declared order, but ordering is only **observably significant for the `Close`↔`FocusPane` pair**: handlers that focus-and-close (jumps from any mode, `Enter` from filter, `Enter`/`l` from command) MUST emit `[Effect::Close, Effect::FocusPane(id)]` in that order so that `hide_self()` precedes `focus_terminal_pane()`, matching today's sequence in `src/main.rs` (and carrying forward the existing macOS focus bug). All other effects (`Save`, `Render`, `Noop`) are commutative — the shim collects flags and applies them once after the loop, so their relative position in the Vec doesn't matter. Handlers may emit them in any order, but for code review consistency the convention is `[Save?, Render?, Close?, FocusPane?, ...]` (mutations → visibility → transitions).

**Effect deduplication**: `Effect::Save` is applied only when `persistence.has_changed()` (no-arg) returns true; the shim wraps the save in this guard so saturating `K`/`J` no-ops or any other state-unchanged effect doesn't write to disk. (`save_if_changed()` is the single canonical entry point; both key dispatch via `Effect::Save` and non-Key event handlers call it.)

**Rationale**: This change is driven by the testability gate. The current `update()` calls zellij FFI inline, making it impossible to unit-test the key dispatch logic without mocking the entire host. By returning `Vec<Effect>` from pure handlers, unit tests can construct a `State`, feed it synthetic `BareKey` events, and assert on the returned effects without touching wasm host APIs. The cost is a one-time refactor of the existing dispatch; the payoff is real test coverage for the mode state machine, slot mapping, K/J reorder, and filter selection logic.

**Alternatives considered**:
- Keep the handlers stateful and rely on integration smoke tests: rejected; smoke tests are not running anywhere automated and rot quickly.
- Use a trait-object for the host (`trait Host { fn focus(&self, id: u32); ... }`) and inject a mock: heavier ceremony, equivalent test power.
- Single ordered effect like `CloseThenFocusPane(u32)` instead of two effects: equivalent power, less granular for tests; the two-effect form lets tests assert each part independently.

### Decision: Workspace split with explicit core boundary

**Choice**: Split the crate into a workspace with two members:
- `harpoon-core` (lib): pure logic — `Mode`, `Config`, `Pane` (host-agnostic projection), `InputKey` (host-agnostic key abstraction), matcher implementations, slot mapping, key handlers, `Effect` enum, `reconcile_selected`, render-string builders. **NO `zellij-tile` dependency**. Builds for native and wasm.
- `harpoon-plugin` (cdylib, wasm32-wasip1): `ZellijPlugin` impl, FFI-typed wrappers (`zellij_tile::PaneInfo` → `harpoon_core::Pane` conversion), render layer (depends on `zellij-tile::Text`/`color_indices`/`color_range`/`print_text_with_coordinates`), persistence I/O. Imports `harpoon-core`.

The cdylib crate keeps the package name `harpoon` so the built artifact path remains `target/wasm32-wasip1/release/harpoon.wasm`; only the workspace member directory name differs.

**Boundary types** (defined in `harpoon-core`):
```rust
pub struct Pane {
    pub id: u32,            // PaneInfo.id
    pub tab_name: String,
    pub pane_title: String,
    pub tab_position: u32,
}

pub enum InputKey {
    Char(char, ModifierSet),  // ModifierSet = bitset of Ctrl/Alt/Shift/Super
    Backspace,
    Esc,
    Enter,
    ArrowUp,
    ArrowDown,
    Other,
}

pub struct ModifierSet { ctrl: bool, alt: bool, shift: bool, super_: bool }
impl ModifierSet {
    /// True iff no modifier is set.
    pub fn is_plain(&self) -> bool { !self.ctrl && !self.alt && !self.shift && !self.super_ }
    /// True iff at most Shift is set (no Ctrl/Alt/Super).
    pub fn is_plain_or_shift(&self) -> bool { !self.ctrl && !self.alt && !self.super_ }
}
```

Note: after FFI normalization (see modifier decision), ASCII letter inputs never carry the Shift bit by the time they reach handlers. So `is_plain()` is sufficient for letter/digit gating. `is_plain_or_shift()` exists primarily for spec readability and any future case where the host emits Shift on non-letter chars.

**Event-local context for handlers**: `a`/`A` (command-mode add operations) need event-local data not in `DispatchState`. Filter-mode `Enter` and arrow nav need the current filtered view. The plugin shim builds and passes both via `DispatchContext`:

```rust
pub struct DispatchContext {
    pub focused_pane: Option<Pane>,
    pub visible_panes: Vec<Pane>,        // already sorted (tab_position ASC, PaneInfo.id ASC)
    pub filtered_indices: Vec<usize>,    // current filtered view (panes idx, score-ordered with tie-break)
}

pub fn dispatch(state: &mut DispatchState, ctx: &DispatchContext, key: InputKey) -> Vec<Effect>
```

`DispatchContext` is built at the FFI boundary on every `Event::Key` from the most recent `PaneManifest`, `TabInfo`, and a one-shot matcher pass over `state.panes` against `state.query`. Sorting visible panes and computing filtered indices happen in the shim (bounded by pane count and matcher cost), so handlers receive ready-to-consume data and can mutate `state.panes`/`state.selected` correctly without calling FFI or driving the matcher themselves. The shim then re-runs `filtered_indices` after handler completion only if a re-render is requested (most key presses don't change the view).

**Filter handler matcher access**: filter `Enter` and arrow nav use `ctx.filtered_indices[state.selected]` to resolve the underlying pane id. They do NOT take a `MatcherImpl` argument — the shim has already done the matching.

The plugin shim converts `zellij_tile::Key { bare_key, key_modifiers }` to `harpoon_core::InputKey` at the FFI boundary, and converts `zellij_tile::PaneInfo` + `zellij_tile::TabInfo` into `harpoon_core::Pane` at the same boundary. Conversion is one ~30-line `From`/match block per direction.

**Rationale**: The repo currently pins `target = "wasm32-wasip1"` in `.cargo/config.toml`, which would defeat plain `cargo test`. The workspace split moves the pin into `harpoon-plugin/.cargo/config.toml` so the root has no pin and `cargo test -p harpoon-core` runs natively. Defining host-agnostic projections (`Pane`, `InputKey`) inside `harpoon-core` keeps the borrow contract clean: handlers own `&mut Vec<Pane>` (not `&mut Vec<zellij_tile::Pane>`), so dispatch unit tests construct test fixtures without touching `zellij-tile`. The conversion cost at the FFI boundary is a deliberate trade.

**Why not `Box<dyn Matcher>` for the matcher**: see the matcher decision below — `Box<dyn Matcher>` adds a `Default` headache on `State` and an unnecessary heap allocation per match call. We use `enum MatcherImpl` for static dispatch.

**Alternatives considered**:
- Single crate with `#[cfg(target_arch = "wasm32")]` guards: simpler restructure but every helper needs the cfg dance, and modifier/key conversion has to live in `cfg`'d code. Rejected for testability.
- Move zellij-typed `Pane` into core (i.e. allow the dep): re-couples the boundary. Rejected.
- Generic handlers (`fn handle_command_key<P: PaneLike>(...)`): infectious type parameters across every handler. Rejected.

**Cost**: One-time `Cargo.toml` restructure, one new directory, `mod` re-wiring, ~30 lines of conversion code. ~45 minutes.

### Decision: Static-dispatch matcher (enum, not Box<dyn>)

**Choice**: Replace `Box<dyn Matcher>` with `pub enum MatcherImpl { Fuzzy(FuzzyMatcher), Substring(SubstringMatcher) }`. The `Matcher` trait still exists for clarity but `State` stores `MatcherImpl` directly, with a `match_indices` method dispatching via `match self { Fuzzy(m) => m.match_indices(...), Substring(m) => m.match_indices(...) }`.

**Rationale**: The `Matcher` trait was originally a `dyn`-friendly interface, but storing `Box<dyn Matcher>` on `State` fights `#[derive(Default)]` (no `Default` for `Box<dyn Trait>`), forces a heap allocation per matcher construction, and adds dynamic dispatch overhead per row. The two-variant enum is functionally equivalent, derives `Default` cleanly (default = `Fuzzy(FuzzyMatcher::default())`), avoids heap allocation, and lets the compiler inline match calls. This is a personal fork with two known matchers; the open-set extensibility of `dyn` is unused.

**Alternatives considered**:
- `Option<Box<dyn Matcher>>` initialized in `load()`: adds an `Option` unwrap on every match call.
- Generic over `M: Matcher`: monomorphizes `State`, which is heavy for a single field.

### Decision: Render builders extracted to harpoon-core

**Choice**: Layout and string-building logic for the header, query line, hint line, and row prefix lives in `harpoon-core` as pure functions returning structured descriptors (e.g. `pub struct RenderRow { pub text: String, pub highlight_indices: Vec<usize>, pub is_selected: bool }` and `pub struct RenderHeader { pub lines: Vec<String>, pub badge_indices: Range<usize>, pub query_indices: Option<Range<usize>> }`). The plugin shim translates these descriptors into `zellij_tile::Text` calls (`color_indices`, `color_range`, `selected`, `print_text_with_coordinates`).

**Rationale**: Without extraction, the highest-risk UI logic (truncation order, hint budgets at 80/50/30 cols, mode-aware hint content, badge row geometry) lives in the plugin crate and is unit-testable only by mocking `zellij-tile`'s `Text`. Extracting the layout decisions into `harpoon-core` lets unit tests assert on `RenderRow`/`RenderHeader` values directly. The plugin's render layer becomes a mechanical translation — thin enough that visual smoke testing covers it.

### Decision: Color index levels and mode-color limitation

**Choice**: zellij-tile `Text` color levels are assigned as: `level=1` for filter match highlights, `level=2` for the mode badge accent, `level=3` retains its existing role for hint-line key labels. Color level `0` stays free.

**Per-mode color accent — limitation**: `zellij-tile`'s emphasis levels (0–3) are theme-driven; there is no API to set a different color *value* per mode at the same emphasis level. Realistically the badge accent renders the same accent color (whatever the theme assigns to level 2) regardless of mode. The mode discriminator therefore comes primarily from the **badge text** (`[N]` / `[F]` / `[J]`), not from a color shift. A true "mode color accent" would require the host to expose per-call color overrides, which `Text::color_range`/`color_indices` do not.

The `mode-state-machine/spec.md` requirement "a mode-distinct color accent is applied to the header" is therefore relaxed in implementation: badges are mode-distinct in *text*; color comes from the theme's level-2 emphasis (uniform across modes). A spec-text update reflects this.

**Rationale**: Without explicit assignment the executor will guess and may collide highlights with hints. Three levels match the three visual concerns; level 0 stays free. The limitation is honest: zellij-tile's API doesn't support per-mode color at a fixed emphasis level. If a future zellij-tile version exposes per-call color, this decision can be revisited.

### Decision: Persistence schema v2 — envelope with single bookmarks Vec

**Choice**: Upgrade the persistence schema to a top-level JSON envelope wrapping a single `bookmarks` array:

```json
{
  "version": 2,
  "bookmarks": [
    { "tab_name": "work",  "pane_title": "nvim", "index": 0 },
    { "tab_name": "shell", "pane_title": "edit", "index": 1 },
    { "tab_name": "build", "pane_title": "cargo", "index": null }
  ]
}
```

Each entry's `index` is `Some(i)` for saved-position placement on next reload, or `None` for append-on-resolve. There is no `materialized` vs `pending` split — the single Vec carries both via the `index: Option<u16>` semantic.

On load, the envelope is read into `Persistence::bookmarks`. Resolution proceeds across `update_panes` rounds per the single-Vec restore model decision: each visible bookmark's pane is placed at `index = Some(i)` or appended at `index = None`.

`PaneBookmark.index: Option<u16>` (not `u16` with `#[serde(default)]`) so v1 detection works via the envelope-vs-bare-array discriminator and the field absence is explicit.

**Why a schema change is required**: `match_pending_bookmarks` is called from `update_panes` and resolves bookmarks **incrementally** — only bookmarks whose `(tab_name, pane_title)` are currently visible in the `PaneManifest` resolve in any given call; the rest stay in `pending_bookmarks` and resolve on later events. Today, `sort_panes()` runs after each `extend(new_panes)` and recovers a deterministic order via `tab_info.position`. After this change removes that sort to make manual order canonical, late-resolving bookmarks would otherwise be appended to the end of `State.panes` rather than placed at their saved position, silently breaking the headline reorder-survives-reload guarantee. The saved index is the simplest reliable fix.

**Migration**: On load, if a bookmark file is read in v1 format (no `index` field), assign indices in array order (today's effective behavior) and mark for re-save. First save after load writes v2.

**Limitation**: Identity is still `(tab_name, pane_title)` for the actual match step. Two panes with identical title in identical tab cannot be distinguished across reloads, so their relative order may swap. This is documented in `specs/reorder/spec.md` as an explicit out-of-scope limitation.

**Alternatives considered**:
- Keep schema unchanged and accept that staggered restore breaks ordering: rejected — the headline guarantee fails on the first multi-tab session reload.
- Add a stable secondary discriminator like `terminal_command` or geometry: too brittle (commands are not always available; geometry changes).
- Sort the union by saved-index in-memory after each extend without persisting an index: requires the index to live somewhere; persisting is the simplest place.

### Decision: Two-helper selection model (focus-anchor + view-clamp)

**Choice**: Replace the single overloaded `reconcile_selected` with two focused helpers in `harpoon-core`:

```rust
/// Anchor `selected` to the focused pane's index when the mode-aware gate passes.
/// Gate: mode == Command || (mode == Filter && query.is_empty()).
/// On gate-pass: selected = focused_idx.unwrap_or(0).min(panes.len().saturating_sub(1)).
/// On gate-fail: selected unchanged.
pub fn reanchor_selected_to_focus(state: &mut DispatchState, focused_idx: Option<usize>);

/// Clamp `selected` against the current visible view length, regardless of mode.
/// Caller passes view_len: panes.len() in command/jump mode, or filtered_indices.len() in filter mode.
pub fn clamp_selected_to_view(state: &mut DispatchState, view_len: usize);
```

Callsites:

| Callsite | Calls |
|---|---|
| `update_panes()` after a pane mutation | both — `reanchor_selected_to_focus` then `clamp_selected_to_view` |
| `close_helper()` | `reanchor_selected_to_focus` (close resets to default mode; view length is `panes.len()`) |
| Filter handler after query mutation | `clamp_selected_to_view(state, ctx.filtered_indices.len())` |
| Filter handler Esc-clear | `reanchor_selected_to_focus` (parity with backspace-clear) |

**Rationale**: The earlier `reconcile_selected` tried to do both jobs but its signature could only access `panes.len()`, not the filtered view length — the helper's promise to "always clamp against current view length" was unimplementable from its arguments. Splitting into two helpers makes each obligation honest and shifts the filtered-view length to the caller (which is the shim, where the view is already computed).

Unit tests: `reanchor_selected_to_focus` is testable with just a `DispatchState` fixture; `clamp_selected_to_view` takes a literal `view_len` and verifies the mode-agnostic clamp. Both are pure and need no `zellij-tile` types.

### Decision: Filter ordering tie-breaker

**Choice**: When two filtered rows have equal match score, the row with the smaller original `panes` index sorts first. The full ordering key is `(-score, panes_index)`, ascending.

**Rationale**: Without a tie-breaker, equal-score matches may swap between renders depending on the matcher's internal iteration order, making `selected = 0` (the top-match-snap behavior) point at a different pane on consecutive keystrokes when the score is unchanged. Original-index tie-break is stable and preserves the user's manual K/J ordering as a secondary signal.

### Decision: `A` add-all deterministic order

**Choice**: When `A` (add all visible panes) executes, panes from the current `PaneManifest` are appended in the order: tab `position` ASC, then within each tab `PaneInfo.id` ASC.

**Rationale**: `PaneManifest.panes` is a `HashMap<usize, Vec<PaneInfo>>`. HashMap iteration order is non-deterministic across runs in Rust. Without explicit ordering, repeated `A` runs append panes in different sequences, producing flaky tests and visible inconsistency after the change removes `sort_panes()`. Iterating tab positions ascending matches the user's spatial mental model (left-to-right tabs). The per-tab `Vec<PaneInfo>` order emitted by zellij is host-implementation-defined and may change across zellij versions; sorting by `PaneInfo.id` (monotonically assigned by zellij) gives a guaranteed-stable order regardless of host quirks.

### Decision: Modifier-gated key consumption with FFI normalization

**Choice**: The FFI conversion from `zellij_tile::Key` to `harpoon_core::InputKey` SHALL normalize ASCII letter inputs as follows: `Char(c) + Shift` where `c` is `'a'..='z'` is rewritten to `Char(c.to_ascii_uppercase()) + ModifierSet { shift: false, .. }`. Other characters and all modifier combinations pass through unchanged. This normalization runs at the FFI boundary (plugin shim, task 3.4) and happens before any handler dispatch.

After normalization:
- `handle_filter_key` appends a printable character to the query ONLY when the input's modifier set is empty `{}`. Inputs with `Ctrl`, `Alt`, or `Super` set are returned as `vec![Effect::Noop]`. (Shift is already collapsed by normalization for letters; for symbols the Shift bit may pass through and is ignored — see symbol-key rule below.)
- `handle_command_key` ASCII-letter and digit arms gate on `modifiers.is_plain()`. Modified variants are no-ops, **with one carve-out**: `c` (close) accepts ANY modifier set. This preserves the today-accidental-but-relied-upon `Ctrl+c` muscle memory for closing the plugin. The carve-out is documented in `specs/mode-state-machine/spec.md` as an explicit scenario.
- `handle_jump_key` slot resolution gates on `modifiers.is_plain()`.
- For shifted symbol keys (`/`, `#`, `?`, `!`, etc.), modifier set is IGNORED in `Command` mode — keyboard layouts vary on whether `#` arrives with implicit Shift, and command mode is content to accept either. In `Filter` mode symbols are always appended (no modifier gate beyond Ctrl/Alt/Super) since they are valid query input.

**Rationale**: The existing fork's command-mode keymap consumes `BareKey::Char` directly without checking modifiers, which means `Ctrl+a` and `a` both fire "add" today. That is a latent bug. In filter mode the surface is *every printable character*, so without modifier gating any `Ctrl+<letter>` becomes silent query input, breaking common terminal expectations (Ctrl+W word-delete, Ctrl+C cancel). The FFI-side shift normalization makes downstream handlers match on a canonical form (`Char('K')` is the only form K reaches the handler in, regardless of whether the host emits `Char('K')` or `Char('k')+Shift`), removing every case-juggling branch in the dispatch core. Phase 0 task 0.3 confirms the host's actual behavior; the normalization is a no-op in one of the two cases.

### Decision: Persistence has_changed covers full persisted shape

### Decision: `state.panes` is a sparse `Vec<Option<Pane>>`, made dense on first user mutation

**Choice**: Replace `state.panes: Vec<Pane>` with `state.panes: Vec<Option<Pane>>`. During the partial-restore window, slots that have a saved-index bookmark but no resolved pane yet are physically represented as `None` at that index; the renderer materializes them as placeholders (per the placeholder-slots decision). On the first user mutation (`a`/`A`/`d`/`K`/`J` with a non-no-op outcome), `state.panes` is **compacted** to drop all `None` entries, becoming effectively dense for the rest of the session.

**Indexing model**:

- During restore window: `state.panes.len()` may exceed the live-pane count; `panes[i]` is either `Some(p)` (live) or `None` (placeholder).
- After first mutation: `state.panes` contains no `None` entries internally; trailing slots simply don't exist (no Vec growth).
- Slot resolution: `slot_index_from_char(c)` maps to a position in `panes`. `panes.get(i).and_then(Option::as_ref)` yields the live pane; `Some(None)` means placeholder; `None` means beyond-list.
- Selection: `state.selected: usize` indexes `panes` directly. Selection MAY land on a `None` (placeholder) during restore; mutation handlers no-op when the selection target is `None`. Navigation (`j`/`k`) does not skip `None` entries — user can highlight a placeholder, but pressing `Enter`/`l`/`d`/`K`/`J` on it no-ops.

**Freeze-on-user-mutation algorithm** (single helper, replaces the prior "rewrite unresolved Some→None" flow):

```rust
// Called as a pre-step from every mutation handler that determined it WILL mutate.
// (Pure no-op handlers never trigger freeze.)
pub fn freeze_on_user_mutation(state: &mut DispatchState, persistence: &mut Persistence) {
    // Track which pane was selected, by id, before compaction.
    let prev_selected_pane_id = state
        .panes
        .get(state.selected)
        .and_then(|opt| opt.as_ref())
        .map(|p| p.id);

    // 1. Rewrite all unresolved Some(_) bookmarks to None (those whose pane id is
    //    not registered in pane_id_to_bookmark_idx).
    let resolved_ids: HashSet<u32> = persistence.pane_id_to_bookmark_idx.keys().copied().collect();
    for b in persistence.bookmarks.iter_mut() {
        if b.index.is_some() {
            // unresolved means no pane currently maps to this bookmark
            let mut is_resolved = false;
            for (&pid, &bk) in &persistence.pane_id_to_bookmark_idx {
                if persistence.bookmarks.get(bk) == Some(b) { is_resolved = true; break; }
            }
            if !is_resolved { b.index = None; }
        }
    }

    // 2. Compact panes (drop None entries).
    state.panes.retain(|opt| opt.is_some());

    // 3. Update each remaining bookmark's saved index to match the new dense position
    //    AND rebuild pane_id_to_bookmark_idx (positions may have shifted).
    persistence.pane_id_to_bookmark_idx.clear();
    for (new_idx, opt) in state.panes.iter().enumerate() {
        if let Some(pane) = opt {
            // Find the bookmark for this pane by (tab_name, pane_title) match
            // limited to bookmarks not yet re-mapped this pass.
            if let Some((bk_idx, b)) = persistence.bookmarks.iter_mut().enumerate()
                .find(|(_, b)| b.tab_name == pane.tab_name && b.pane_title == pane.pane_title
                               && !persistence.pane_id_to_bookmark_idx.values().any(|&v| v == /* this idx */))
            {
                b.index = Some(new_idx as u16);
                persistence.pane_id_to_bookmark_idx.insert(pane.id, bk_idx);
            }
        }
    }

    // 4. Re-anchor selection: if previously on a Some pane, find it in the dense
    //    Vec by id; else clamp to 0.
    state.selected = match prev_selected_pane_id {
        Some(pid) => state.panes.iter().position(|opt| opt.as_ref().map(|p| p.id) == Some(pid)).unwrap_or(0),
        None => 0,
    };
}
```

(The `bookmarks.iter_mut().enumerate().find()` predicate above is illustrative; the production form uses a `HashMap<(String, String), Vec<usize>>` lookup table built once at the top of step 3 to avoid quadratic scan with duplicates. See task 4.0 for the concrete impl.)

**Trigger sites**:

- `Char('a', plain)` add focused: BEFORE pushing the new pane, call `freeze_on_user_mutation`, THEN push.
- `Char('A', plain)` add all: same — freeze first, then iterate-and-append.
- `Char('d', plain)` delete: only call freeze if `panes[selected].is_some()`; otherwise pure no-op.
- `Char('K')` shift up: only call freeze if `panes[selected].is_some() && selected > 0 && panes[selected-1].is_some()` (post-compaction these are always true if selected was on a Live pane); else no-op.
- `Char('J')` shift down: same logic, mirrored.
- Navigation (`j`/`k`/arrows), mode transitions (`/`/`#`/`Esc`), focus (`Enter`/`l`), and slot-jumps (which only `Effect::Close + FocusPane`) do NOT freeze — they don't mutate `Persistence::bookmarks` or `state.panes` ordering.

**Rationale**: The owner picked Alt-A (placeholder slots preserve saved positions during restore) over collapsed-index. Honoring that contract requires either physical gaps in `panes` or a saved-index→panes-index translation layer. Sparse `Vec<Option<Pane>>` makes the gaps the in-memory truth: `panes[2]` is `None` if slot 2 is a placeholder, period. Every slot resolution, render, and selection lookup goes through `panes.get(i).and_then(Option::as_ref)` — one mechanical pattern that's hard to forget. The mapping-layer alternative would put a translation between every handler and `panes`, multiplying touch points.

Freeze-on-first-mutation collapses the sparse representation to dense the moment user-driven order matters, so the rest of the session uses the simpler dense indexing. The complexity is bounded to the restore window.

**Alternatives considered**:
- *Mapping layer* (saved_index → panes_index lookup, panes stays dense): keeps `panes` simple but every handler needs the translation. Rejected — multiplies touch points.
- *Collapsed-index* (revert D4): simplest code; breaks the saved-position contract during restore window. Rejected by owner in Step 6.
- *Two separate Vecs* (`live: Vec<Pane>`, `placeholders: Vec<Placeholder>`): forces every render and slot lookup to merge two sources. Rejected — the merge logic is itself the bug surface.

### Decision: `Persistence::pane_id_to_bookmark_idx` is a first-class map

**Choice**: Add `pane_id_to_bookmark_idx: HashMap<u32, usize>` as a field on `Persistence`. The map is the authoritative pane↔bookmark identity tracker. Maintenance points:

- On `a` (add focused): after pushing the bookmark, insert `pane.id → bookmarks.len() - 1`.
- On `A` (add all): for each newly added pane, same insert.
- On `d` (delete): remove entry by `pane.id`; remove the matching bookmark; for any bookmark with `index = Some(j)` where `j > deleted_index`, decrement to `Some(j - 1)`; rebuild remaining entries in the map (their bookmark indices shifted).
- On `K`/`J` (reorder): swap pane positions in `state.panes`; swap the corresponding bookmarks' `index` fields; the map's `pane_id → bookmark_idx` entries do NOT change because bookmarks don't move within `Persistence::bookmarks` — only their `index` fields update.
- On restore resolution (a `Pane` materializes from a bookmark): insert `pane.id → bookmark_idx`.
- On freeze: the freeze algorithm rebuilds the map from scratch (per the algorithm above).
- On startup load: empty map; populates as bookmarks resolve.
- On a pane disappearing from `PaneManifest` (existing `get_valid_panes` filter): remove the map entry by `pane.id`. The bookmark itself remains in `Persistence::bookmarks` so reload-after-respawn-with-same-name behavior matches today's existing fork (consume-on-first-resolve semantics).

**Rationale**: The R9 review correctly identified that K/J/d/freeze/`is_resolved`/placeholder rendering all need a primitive that answers "which bookmark corresponds to this live pane?" The previous design hand-waved this as "HashMap or iterate pairs each round" — the iterate-pairs option fails on duplicate `(tab_name, pane_title)`. Promoting the map to a first-class field makes maintenance points explicit and testable.

**Alternatives considered**:
- *Iterate pairs each round*: O(N) per query, fails on duplicates. Rejected.
- *Embed bookmark_idx on Pane*: couples `harpoon-core::Pane` to persistence concerns. Rejected.

### Decision: Persistence has_changed covers full persisted shape

**Choice**: `Persistence::bookmarks: Vec<PaneBookmark>` is the canonical in-memory mirror of disk state. `save_to_disk` writes `PersistedV2 { version: 2, bookmarks: self.bookmarks.clone() }` with no rebuild step. `last_saved_state: Option<PersistedV2>` holds the last-saved envelope for diff. Signature:

```rust
impl Persistence {
    pub fn has_changed(&self) -> bool { ... }            // compares self.bookmarks vs last_saved_state.bookmarks
    pub fn save_if_changed(&mut self) -> Result<(), Error> { ... }
}
```

Note `has_changed` and `save_if_changed` take **no arguments** — both compare and write `self.bookmarks` directly. Callers (key dispatch via `Effect::Save`, non-Key event tail saves) keep `self.bookmarks` in sync with `self.panes` AS THEY MUTATE. Specifically:

- `a` (add focused): push `Pane` into `state.panes` AND push matching `PaneBookmark { ..., index: Some(panes.len() - 1) }` into `Persistence::bookmarks`.
- `A` (add all): for each newly added pane, push the corresponding bookmark with the new index.
- `d` (delete at i): remove from both `state.panes` and `Persistence::bookmarks`; decrement `index` for any subsequent bookmark entries whose `index = Some(j)` with `j > i`.
- `K`/`J` (reorder): swap entries at `selected` and `selected ± 1` in BOTH `state.panes` and `Persistence::bookmarks`; update both swapped bookmarks' `index = Some(new_position)`.
- Restore resolution (in `update_panes`): when a bookmark with `index = Some(i)` becomes visible, materialize the matching `Pane` into `state.panes` at index `i`. The bookmark itself stays in `Persistence::bookmarks` unchanged (it's already at its correct disk shape).
- Freeze on user mutation during partial restore: rewrite unresolved `Some(_)` → `None` in `Persistence::bookmarks`. The on-disk shape automatically reflects the freeze on next save.

**Crucially, `save_if_changed` always serializes the COMPLETE `Persistence::bookmarks` Vec — including unresolved entries with `index = Some(_)` that haven't yet materialized into `state.panes`**. This was a P0 regression in the earlier rebuild-from-panes design: a `save_if_changed` call during partial restore (e.g. user reorders `K`/`J` while another bookmark is still pending) would have truncated the unresolved bookmarks because they didn't exist in `panes`. Disk would lose them. Treating `Persistence::bookmarks` as the canonical shape and never rebuilding it from `panes` makes the disk write trivially complete.

This avoids the borrow-check trap of `self.persistence.save_if_changed(&self.panes, &self.persistence.pending)` (immutable + mutable borrow of the same `Persistence`) — neither argument is needed.

**Rationale**: After Round-5 audit, the original `Vec<(tab_name, pane_title)>` shape couldn't detect duplicate-title reorder or post-freeze append-list changes. After Round-6 collapse to a single `bookmarks: Vec<PaneBookmark>`, the comparison reduces to a single-shape diff. The R8 audit caught that rebuilding the candidate from `panes` truncates unresolved entries; making `Persistence::bookmarks` the canonical shape and keeping it in sync at every mutation closes that hole. Comparison is `PartialEq` on `PersistedV2`; bytewise serde_json fallback is the cheapest if derive contention arises.

### Decision: Schema version detection via JSON envelope shape

**Choice**: v1 vs v2 is detected by the top-level JSON shape: if the file deserializes as a bare `Vec<PaneBookmark>` (no `version` field), it is v1; if it deserializes as the envelope `{ "version": 2, "bookmarks": [...] }`, it is v2. The single-Vec `bookmarks` shape (no `materialized` vs `pending` split) is the canonical v2 form per the single-Vec restore decision.

Load code attempts v2 envelope first; on `serde_json` parse error, falls back to attempting v1 bare-array; on second failure, logs `LoadFromDiskFailed` and starts with an empty bookmarks Vec. v1 detection then assigns `index = Some(i as u16)` in array order; the next save naturally writes v2 envelope.

**Rationale**: A primary discriminator (envelope vs array) avoids any heuristic on field values. The earlier `#[serde(default)] + all-zero` heuristic aliased legitimate single-entry v2 files; a presence-based check on `Option<u16>` was a step better but still required treating v1's bare-array and v2's envelope as the same JSON top-level (which they aren't). The envelope makes v2 forward-compatible (future versions can add fields like `last_used_mode` without re-shaping).

### Decision: Render header metadata is per-line

**Choice**: `RenderHeader` is structured as `Vec<HeaderLine>` rather than `Vec<String>` plus global ranges. Each `HeaderLine { text: String, badge_range: Option<Range<usize>>, query_range: Option<Range<usize>> }` carries its own annotations.

**Rationale**: When narrow-width truncation moves the badge to a separate line, a single set of `badge_char_range` / `query_char_range` ranges cannot disambiguate which line owns which range. Per-line metadata keeps the renderer explicit: line 0's badge_range applies to line 0's text, line 1's query_range applies to line 1's text. Tests assert per-line content directly.

### Decision: Tiny-pane layout precedence

**Choice**: At small heights, layout elements drop in this priority (most-droppable first): hint line at `rows-1` is dropped first; then header collapses to single-line layout (no narrow-width 2-line badge layout); then header itself is dropped if `rows < 2`. The pane-row list is the highest-priority element and is never preempted.

**Rationale**: Without precedence, the row budget computation `rows.saturating_sub(2).saturating_sub(header.height)` can produce zero rows on tiny panes (e.g. `rows = 4` with a 2-line filter header → 0 row slots, user sees no list). Dropping the hint line first preserves at least one row of pane data; dropping a 2-line header second keeps things usable down to ~3 rows; dropping the header below 2 rows is a degenerate case but allowed.

**Implementation**: `build_header(state, cols)` accepts a `max_height` parameter; when supplied, narrow-width truncation may need to fall back to single-line. The plugin shim computes `max_header_height = if rows >= 4 { 2 } else { 1 }` and passes that to `build_header`.

### Decision: Workspace cargo command form (canonical)

**Choice**: The plugin package is named `harpoon` (preserved across the rename so artifact path stays `harpoon.wasm`). The new lib package is `harpoon-core`. All cargo commands use the **package name** via `-p <name>`.

**Cargo config inheritance** (the rule that bit Round 6): cargo walks `.cargo/config.toml` from the **current working directory** upward, NOT from the package's directory. Therefore:

- From workspace root with `-p harpoon`: cargo does NOT pick up `harpoon-plugin/.cargo/config.toml`. Build runs against the host's default target unless `--target wasm32-wasip1` is explicit.
- From `harpoon-plugin/`: cargo walks `harpoon-plugin/.cargo/config.toml` and picks up the wasm pin.

**Canonical commands**:

| Purpose | Command | Cwd |
|---|---|---|
| Build plugin (wasm) | `cargo build --release` | `harpoon-plugin/` |
| Build plugin (wasm), root form | `cargo build -p harpoon --target wasm32-wasip1 --release` | workspace root |
| Test core (native) | `cargo test -p harpoon-core` | workspace root |
| Plugin native tests | NONE — FFI conversion tests live in `harpoon-core` (see below) | n/a |
| Lint | `cargo clippy --workspace --all-targets` | workspace root |

**Plugin-crate native tests intentionally absent**: FFI conversion tests are placed in `harpoon-core` against primitive-typed shims. `harpoon-core` exposes `pub fn parse_input_key(bare_key_kind: &str, c: Option<char>, ctrl: bool, alt: bool, shift: bool, super_: bool) -> InputKey` taking primitive arguments; the plugin's `From<zellij_tile::Key>` is a thin wrapper delegating to this function. Tests target `parse_input_key` directly without needing `zellij-tile` types or a native target on the plugin crate.

**Rationale**: Earlier rounds mistakenly described the per-crate config as inherited from root. Cargo's actual behavior requires either `cd` into the plugin crate or an explicit `--target` flag from root. Documenting this explicitly avoids a build-time surprise during execution.

### Decision: Restore resolution algorithm (sparse Vec, concrete)

**Choice**: The restore resolution function lives in `harpoon-core` so it is unit-testable without `zellij-tile`. Signature:

```rust
pub fn resolve_restore_round(
    bookmarks: &mut Vec<PaneBookmark>,
    pane_id_to_bookmark_idx: &mut HashMap<u32, usize>,
    panes: &mut Vec<Option<Pane>>,           // sparse during restore window
    visible: &[(String, String, u32, u32)],  // (tab_name, pane_title, pane_id, tab_position)
);
```

The function is invoked from `update_panes()` (after the visible-panes slice is constructed from `PaneManifest`+`TabInfo`). Algorithm:

```
// 1. Grow panes to accommodate any saved indices not yet covered.
let max_saved_idx = bookmarks.iter().filter_map(|b| b.index).max().unwrap_or(0);
if panes.len() < max_saved_idx as usize + 1 {
    panes.resize(max_saved_idx as usize + 1, None);
}

// 2. Build a per-round consumption set so duplicate-titled bookmarks distribute across
//    duplicate-titled visible panes (first-match-wins; consume per-round).
let mut consumed_visible_ids: HashSet<u32> = HashSet::new();

// 3. Walk bookmarks in stable order.
for (bk_idx, b) in bookmarks.iter().enumerate() {
    // Skip already-resolved bookmarks (their pane.id is in the map).
    if pane_id_to_bookmark_idx.values().any(|&v| v == bk_idx) {
        continue;
    }
    // Find a visible pane matching this bookmark, not yet consumed this round.
    let matched = visible.iter()
        .find(|(tn, pt, id, _)| tn == &b.tab_name && pt == &b.pane_title && !consumed_visible_ids.contains(id));
    if let Some((tn, pt, id, tp)) = matched {
        consumed_visible_ids.insert(*id);
        let p = Pane { id: *id, tab_name: tn.clone(), pane_title: pt.clone(), tab_position: *tp };
        match b.index {
            Some(i) => {
                // Place at saved index; gap was pre-allocated by the resize above.
                panes[i as usize] = Some(p);
            }
            None => {
                // Append at end; rewrite this bookmark's index to its new dense position.
                panes.push(Some(p));
                // Note: bookmarks borrow is conflicted here; in practice the impl
                // collects these (bk_idx, new_idx) pairs into a side-buffer and
                // applies them in a second pass. See task 8.4 for the working form.
            }
        }
        pane_id_to_bookmark_idx.insert(*id, bk_idx);
    }
    // else: bookmark not yet visible; renders as a placeholder via build_rows seeing None at panes[b.index.unwrap()].
}

// 4. Apply None-bookmark index rewrites collected in step 3 (deferred to avoid borrow conflict).
```

**Resolved-state tracking**: a bookmark is considered resolved when an entry in `pane_id_to_bookmark_idx` points to it. The map is the authoritative tracker (see the `pane_id_to_bookmark_idx` first-class map decision). On `a`/`A` user adds, the new bookmark is APPENDED to `Persistence::bookmarks` AND the map is populated atomically, so subsequent rounds see it as resolved.

**Sparse semantics**: `panes` is a `Vec<Option<Pane>>` throughout the restore window. `Some(_)` slots are live; `None` slots are placeholders rendered via `build_rows` (which pulls placeholder text from the matching `Persistence::bookmarks` entry by `index = Some(slot)`). Slot keys on `None` slots no-op.

**Freeze**: separate from resolution. See `freeze_on_user_mutation` (task 4.0) — invoked as a pre-step from each mutation handler. Freeze compacts `panes` to dense, rewrites unresolved `Some(_)` bookmarks to `None`, rebuilds the map, and re-anchors `selected`. Resolution can continue to fire on subsequent `update_panes` events; post-freeze, late-resolving bookmarks now have `index = None`, so they append to the (now-dense) `panes` Vec rather than seek their old saved positions.

**Rationale**: Earlier rounds described the restore behavior in prose without committing to a concrete loop. The R9 review surfaced that the dense-Vec `insert(min(i, panes.len()), p)` algorithm could not honor the saved-position contract during partial restore (gaps would collapse). The sparse algorithm above places resolved panes at their literal saved index in the Vec, leaving `None` placeholders in the gaps until resolution completes.

### Decision: Single-Vec restore model (collapses restore_buffer + pending_late_resolve)

**Choice**: There is exactly one persistent collection on `Persistence` driving the restore lifecycle:

```rust
// in src/persistence.rs
pub struct Persistence {
    bookmarks: Vec<PaneBookmark>,           // single source of truth
    last_saved_state: Option<PersistedV2>,  // for has_changed comparison
    // ... existing fields
}

pub struct PaneBookmark {
    pub tab_name: String,
    pub pane_title: String,
    pub index: Option<u16>,  // Some(i) = place at saved index; None = append on resolve
}
```

Resolution semantics on each `update_panes` round walk `Persistence::bookmarks` once and produce `State.panes` (which is `Vec<Option<Pane>>` — sparse during the partial-restore window):

1. **Pre-size**: ensure `state.panes.len() >= max_saved_idx + 1` by `state.panes.resize(max_saved_idx + 1, None)`, where `max_saved_idx = bookmarks.iter().filter_map(|b| b.index).max().unwrap_or(0)`. This pre-allocates `None` slots for unresolved saved-index bookmarks; resolution writes `Some(p)` at the exact saved index without shifting other entries.
- For each bookmark with `index = Some(i)` whose `(tab_name, pane_title)` is currently visible: write `state.panes[i] = Some(p)` directly (no clamping needed because step 1 pre-sized the Vec). Insert `pane.id → bookmark_idx` into `pane_id_to_bookmark_idx`.
- For each bookmark with `index = None` whose `(tab_name, pane_title)` is currently visible: push `Some(p)` to the end of `state.panes`; rewrite the bookmark's `index` field to `Some(state.panes.len() - 1)` (the new dense position).
- Bookmarks not yet visible remain in `Persistence::bookmarks` unchanged. Their saved-index slots remain `None` in `state.panes` and render as placeholders via `build_rows`.

**Freeze-on-user-mutation**: when the user presses `a`/`A`/`d`/`K`/`J` while `Persistence::bookmarks.iter().any(|b| b.index.is_some() && !is_resolved(b))` (i.e. saved-index restore is incomplete), every still-unresolved bookmark with `index = Some(_)` has its index rewritten to `None`. Subsequent late-resolves of those bookmarks go via append, not saved-index placement.

### Decision: Placeholder slots during partial restore (Alt-A)

**Choice**: When `Persistence::bookmarks` contains entries with `index = Some(i)` that have NOT yet materialized into `state.panes` (i.e. their `(tab_name, pane_title)` has not appeared in any `PaneManifest` yet), the rendered list SHALL display **placeholder rows** at the saved indices. Saved positions are visible in the row layout; live panes occupy their saved indices; gaps are filled with `<slot>  ?  (resolving)` rows.

**Render-side derivation** (in `harpoon-core::build_rows`): the renderer is given a `Vec<RowEntry>` of length `max(state.panes.len(), max_saved_index + 1)`, where each entry is either `RowEntry::Live(&Pane)` or `RowEntry::Placeholder { saved_tab_name: String, saved_pane_title: String }`. The plugin shim builds this Vec by walking `Persistence::bookmarks` and intersecting with `state.panes`:

```
for each bookmark b in Persistence::bookmarks:
    if b.index = Some(i) AND b is unresolved:
        // bookmark not in pane_id_to_bookmark_idx, so this slot has no live pane yet
        // (slot index is b.index.unwrap() since this branch already filtered for Some)
        // — nothing to do; build_rows will see state.panes[b.index.unwrap()] = None
        // and call placeholder_lookup(b.index.unwrap()) to get the saved metadata.
```

The sparse `state.panes` Vec IS the row source: `Some(p)` at index `i` renders as a Live row; `None` at index `i` renders as a Placeholder (with metadata pulled from the unresolved bookmark whose `index = Some(i)`). The render-side `build_rows` (task 7.3) walks `state.panes` directly:

```
for i in 0..state.panes.len():
    match &state.panes[i] {
        Some(p) => rows.push(Live(p)),
        None => rows.push(Placeholder { ... from placeholder_lookup(i) }),
    }
```

Once a placeholder resolves, `state.panes[i]` flips from `None` to `Some(p)` and the next render naturally shows a Live row at the same index.

**Slot-key behavior on placeholders**: pressing a slot key (digit or letter) that resolves to a `Placeholder` row in either `Command` or `Jump` mode SHALL be a **no-op** (return `vec![]`, no Render, no Close). The user's saved-position muscle memory survives the restore window: pressing `2` jumps to whatever the user pinned at slot 2, OR no-ops if slot 2 hasn't resolved yet — it never jumps to the wrong pane.

**Filter mode**: placeholders are excluded from the filtered view entirely. Filter operates on live panes only; resolving panes appear in the filter results once they materialize.

**Freeze interaction**: when freeze rewrites unresolved `Some(_)` → `None`, those entries STOP rendering as placeholders (they no longer have a saved index). They go back to append-on-resolve semantics and appear at the end of the list when they eventually become visible.

**Rationale**: chosen by owner over the simpler collapsed-index alternative. The user's mental model is "saved position = slot number forever", and the collapsed-index approach silently broke that during the (typically sub-second) restore window. The placeholder approach preserves the contract: slot numbers are stable through restore, including during the gap. The cost is `~30 LOC` of `RowEntry` enum + render-side gap-fill logic. The `?  (resolving)` text doubles as user-visible feedback when the restore window is unusually long (e.g. tab tree not yet open).

**Alternatives considered**:
- *Collapsed-index* (originally chosen, then rejected by owner): only show resolved panes; rebuild slot numbers as bookmarks resolve. Simplest code; breaks slot-position contract during restore window.
- *Gate input until restore complete*: show "restoring…" and ignore keys until all bookmarks resolved (or timeout). Safest; heavy-handed for typically-short window.
- *Show placeholder names but allow jumps anyway* (jumps to nothing): bad UX (silent fail).

**Out of scope**: a long-running placeholder might want a timeout to flip to `None` automatically ("give up restoring after 10s"). Not pursued; the freeze-on-mutation rule already provides an escape hatch.

**Rationale**: The earlier two-collection design (`restore_buffer: Option<Vec<RestoreSlot>>` + `pending_late_resolve: Vec<PaneBookmark>`) introduced contract drift across artifacts — spec scenarios disagreed with task descriptions about which collection persists across reload, where saved indices live, and how the on-disk envelope splits the two. Collapsing into one `Vec<PaneBookmark>` with `index: Option<u16>` semantics removes the ambiguity entirely:

- Saved-index placement: `index = Some(i)`.
- Append: `index = None`.
- Persisted shape is just `Vec<PaneBookmark>` (no `materialized` vs `pending` split). The v2 envelope wraps it for forward-compatibility (`{ version: 2, bookmarks: [...] }`).
- Reload after freeze: the formerly-frozen entries persisted with `index = None`, so they re-enter the restore lifecycle but with append semantics, not their old saved positions.

**Save shape**: `save_to_disk()` (no-arg) writes `PersistedV2 { version: 2, bookmarks: self.bookmarks.clone() }` directly. **No rebuild step from `state.panes`** — `Persistence::bookmarks` is the single source of truth, kept in sync at every mutation site (see the `pane_id_to_bookmark_idx` decision). Unresolved entries with `index = Some(_)` (saved-position bookmarks awaiting late resolution) are persisted faithfully.

This is documented in `specs/reorder/spec.md` with scenarios.

### Decision: cargo target pin scope

**Choice**: The repo-level `.cargo/config.toml` (currently pinning `wasm32-wasip1` for the whole crate) is moved to `harpoon-plugin/.cargo/config.toml` after the workspace split. The workspace root has no target pin. From the workspace root: `cargo test -p harpoon-core` runs natively; plugin wasm builds require either `cd harpoon-plugin && cargo build --release` OR `cargo build -p harpoon --target wasm32-wasip1 --release` from root — cargo does NOT walk `harpoon-plugin/.cargo/config.toml` when invoked from the workspace root. Tests in `harpoon-plugin` (if any are added later) would need explicit `--target <native>` or be excluded — this change does not add plugin-crate tests, so the question is deferred.

Note: package name is `harpoon`, NOT `harpoon-plugin` (the latter is the directory name only). All `cargo` invocations use `-p harpoon` for the plugin and `-p harpoon-core` for the lib.

**Rationale**: Without this move, even after the workspace split, the inherited workspace target pin would force `cargo test -p harpoon-core` to compile as wasm and fail to execute. Per-crate cargo configs supersede workspace configs only for that crate's directory; moving the file is the cleanest separation.

### Decision: Filter matches the display string excluding the slot prefix

**Choice**: The fuzzy matcher receives `format!("{} | {}", tab_name, pane_title)` (today's `Display` impl), without the slot prefix. Matched-byte indices are then offset by the prefix width when applied via `color_range` during render.

**Rationale**: Otherwise typing `1` would match every row (because every row is prefixed with a slot character). Excluding the prefix keeps filtering semantically clean.

## Risks / Trade-offs

- **Risk**: `nucleo-matcher` doesn't compile for `wasm32-wasip1`. → **Mitigation**: Phase 0 spike validates the build before any mode-system work. In-tree fallback is well-scoped (~50 LOC) and listed as a tasks.md alternate path.
- **Risk**: The mode state machine introduces UX confusion (which mode am I in?). → **Mitigation**: Persistent `[N]`/`[F]`/`[J]` badge plus color accent on every render. Default config keeps users in `command` mode, preserving today's behavior unless they opt in.
- **Risk**: Slot prefixes change row layout, breaking the rendering math for selected highlight or hint line. → **Mitigation**: All x-coordinates remain at `0`; the prefix is part of the `Text` payload, not a separate column. Cursor placement and selected highlight remain row-level, not column-level.
- **Risk**: `selected` semantics become bug-prone — index-into-panes vs index-into-filtered. → **Mitigation**: A small accessor (`fn selected_pane_index(&self) -> Option<usize>`) returns the *underlying-Vec* index regardless of mode, used by all mutating commands (`d`, `K`, `J`, focus). Mode-specific code only touches the raw `selected` field for cursor movement.
- **Risk**: User reorders panes mid-filter (`K`/`J` while in filter mode). The current decision: reorder only fires in command mode, so this can't happen — but the user might be surprised that `K`/`J` does nothing in filter mode (where they're query characters anyway). → **Mitigation**: Documented in spec; no special UI affordance needed because `K`/`J` is consumed as query input in filter mode (same as `a`, `d`, etc.).
- **Risk**: Match-highlight indices drift if the prefix width changes (e.g. `show_slots = false`). → **Mitigation**: Prefix width is a single computed value per render call; indices added at the same site. Trivial as long as we don't carry stale offsets across renders.
- **Risk**: Default mode = `filter` makes `Esc` need three presses to close from a typed query. → **Mitigation**: Documented behavior. `c` is not consumed in filter mode (it's a query char), so users in filter mode wanting one-key close type Esc to drop to command, then `c`. Two keystrokes is the floor.
- **Risk**: `BREAKING` flag on `What Changes` understates how disruptive a non-default `default_mode` is. → **Mitigation**: Default is `command`; users opt into the new behavior. Migration is purely additive.
- **Risk (carried forward)**: Existing `// TODO: hide_self + focus_terminal_pane has a bug on macOS with hidden panes` (in current `src/main.rs`). The new slot-jump and filter-Enter paths add three more callsites that share this bug. → **Mitigation**: All new callsites flow through the same `close()` helper, so any future fix is one-place. This change does not regress macOS behavior; it inherits the existing bug. Owner runs Linux primarily; defer fix.
- **Risk**: `BareKey::Char('K')` / `BareKey::Char('J')` may be delivered as `Char('k')`+shift modifier rather than as separate uppercase chars by some zellij host versions. → **Mitigation**: Existing fork code already uses `BareKey::Char('A')` for `A`-add-all, confirming the host emits uppercase chars for shifted letters in our environment. Tasks include a one-build verification probe before relying on `K`/`J` in production paths.
- **Risk**: `nucleo-matcher` is built with feature flags that interact poorly with `wasm32-wasip1` even if the bare crate compiles. → **Mitigation**: Phase 0 spike builds with `--no-default-features` and only enables the minimum needed (matcher + indices); fallback path is documented.
- **Risk**: y=0 header row is invisible per an existing comment in `src/main.rs` ("Note: y=0 overlaps with the zellij pane frame/title bar and is not visible, so we start rendering from y=1"). The mode badge mitigation for "which mode am I in?" relies on the badge being visible. → **Mitigation**: Phase 0 task verifies whether y=0 is genuinely hidden by rendering a probe string at y=0 and y=1 in a dev workspace. If y=0 is hidden, header/badge/query line render at y=1 and pane rows start at y=2 (or y=2/y=3 for two-line narrow header). `render_header()` returns the consumed row count (`u16`) and the row-rendering loop starts at that offset, so the layout adapts cleanly.
- **Risk**: The plugin instance might NOT survive `hide_self()` (the `close_helper` reset-on-close design assumes it does). If the host re-instantiates the plugin on every open, all close-helper resets are dead code and `default_mode` is moot. → **Mitigation**: Phase 0 task adds an `eprintln!("instance addr: {:p}", self as *const _)` probe in `update()` and `load()`, cycles open/close, confirms the same address. If the assumption fails, redesign by moving mode-init to `load()` and dropping the close-helper reset.
- **Risk**: `match_pending_bookmarks` resolves identity by `(tab_name, pane_title)` only. Two panes with identical title in identical tab (common: two `nvim` panes) cannot be distinguished across reloads, so their manual relative order may swap. → **Mitigation**: Documented as an explicit out-of-scope limitation in `specs/reorder/spec.md` ("Reorder of two panes with identical (tab_name, pane_title) is not preserved"). Solving this fully requires a stable per-pane discriminator that survives reload, which the zellij API does not currently expose for terminal panes.

## Migration Plan

Personal fork, single user. No staged rollout needed. Branch off `main`, implement on a feature branch, validate locally with the `plugin-dev-workspace.kdl` harness, merge to `main`. README documents the new keymap and config keys before merge so reference is available the moment the new build runs.

**Persistence file location** (unchanged from existing fork): the persisted bookmarks live at `${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon/<session_name>.json`, one file per zellij session. The v2 schema applies to each session file independently.

**Persistence v1 ↔ v2 migration**:

- *v2 binary reads v1 file*: v1 files are a bare `Vec<PaneBookmark>` JSON array; v1's `PaneBookmark` has no `index` field. The v2 type defines `index: Option<u16>` so any missing field deserializes to `None`. Load attempts v2 envelope first; on failure, attempts v1 bare-array. v1 success → assign `index = Some(i as u16)` in array order. The next save writes v2 envelope.
- *v2 binary reads v2 file*: top-level JSON envelope `{"version": 2, "bookmarks": [...]}` where each entry's `index` is `Some(i)` for materialized bookmarks (currently in `state.panes`) and `None` for pending late-resolve bookmarks (frozen out of restore, awaiting append).
- *v1 binary reads v2 file*: v1 binary expects a bare `Vec<PaneBookmark>` and fails to deserialize the v2 envelope. Result: v1 loader logs `LoadFromDiskFailed` and starts with empty bookmarks. The v2 file remains on disk untouched.
- *fresh install*: starts at v2 envelope format.

**Rollback consequences**: rolling back from a v2 binary to a v1 binary will cause the v1 binary to fail to read each session's v2 file and start with an empty pinned set. The v2 files are untouched on disk, so re-installing the v2 binary restores everything. The v1 binary will eventually overwrite a session's file the next time it saves — if the user re-pins panes under v1, that save is a v1 bare-array and the v2 envelope is gone. Recommended rollback procedure for any user who has pins they want to keep:

1. Back up the persistence directory before installing v1: `cp -r "${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon" "${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon.v2-backup"`.
2. Revert the merge commit; install v1.
3. If pins must be carried back, manually re-pin in the v1 binary (the v2 file's `(tab_name, pane_title)` are still readable by hand). Or restore from the backup directory and re-install v2.

**Workspace target pin**: after the split, `.cargo/config.toml` (with `target = "wasm32-wasip1"`) lives at `harpoon-plugin/.cargo/config.toml`. **Cargo only walks `.cargo/config.toml` from the current working directory upward**, so a per-crate config is NOT inherited when cargo is invoked from the workspace root. Concretely:

- `cargo test -p harpoon-core` from the workspace root: works — no target inherited (root has no config), runs natively. ✅
- `cargo build -p harpoon` from the workspace root: builds for the host native target (target NOT inherited from `harpoon-plugin/.cargo/config.toml`), and the cdylib link will fail or produce a non-wasm artifact. ❌
- `cd harpoon-plugin && cargo build --release`: works — the cwd is inside `harpoon-plugin/`, so the per-crate config applies and the wasm target is selected. ✅
- `cargo build -p harpoon --target wasm32-wasip1 --release` from the workspace root: works — explicit `--target` overrides any config inheritance question. ✅

**Canonical commands** (always pass explicit `--target` for plugin builds from root, or `cd` into the plugin directory):

| Purpose | Command |
|---|---|
| Test core (native) | `cargo test -p harpoon-core` (from root) |
| Build plugin (wasm) | `cd harpoon-plugin && cargo build --release` *or* `cargo build -p harpoon --target wasm32-wasip1 --release` (from root) |
| Lint workspace | `cargo clippy --workspace --all-targets` (from root; clippy works on all targets without a wasm pin) |

## Open Questions

- **Matcher dep**: Will `nucleo-matcher` build for `wasm32-wasip1` cleanly? Resolved by the Phase 0 spike in `tasks.md`. If no, switch to in-tree.
- **Color palette for mode accents**: Specific zellij color enum values for command/filter/jump. Not blocking; pick during render-layer task. Suggested: blue, yellow, green respectively.
- **Letter slot ordering for `a-z`**: Direct alphabetical (a=10, b=11, ..., z=35). Confirmed in specs but worth re-checking once implemented — alternate dvorak/colemak users may prefer a row-order layout. Out of scope for this change.
