## 1. Risk probes (R2, R3 — evidence before wiring)

- [x] 1.1 Probe R2 (keybind-source pipe delivery/permission): in a scripted
  zellij session (tmux-hosted harness precedent), bind a key to
  `MessagePlugin "file:...harpoon.wasm" { name "toggle"; }`, drive the key,
  and confirm the pipe message reaches the loaded plugin (log line from the
  pipe handler) and whether any permission prompt/denial occurs. Record the
  evidence (delivery path, PipeSource variant observed, permission outcome)
  in review.md Execution Notes.
  - intent: spike
  - files_allowed:
      - scripts/toggle-pipe-probe.sh
      - openspec/changes/pipe-toggle-invocation/**
- [x] 1.2 Probe R3 (suppressed pane visibility in `PaneManifest`): with the
  plugin hidden via `hide_self()`, dump what the plugin's cached
  `PaneUpdate` manifest reports for its own pane (present/absent,
  `is_suppressed` or equivalent). Decide own-tab detection strategy: manifest
  lookup if reliable, else the frozen fallback (unconditional
  show-then-relocate-if-needed). Record decision + evidence in review.md
  Execution Notes.
  - intent: spike
  - files_allowed:
      - scripts/toggle-pipe-probe.sh
      - openspec/changes/pipe-toggle-invocation/**

## 2. Core decision logic (Constitution I)

- [x] 2.1 Add pure toggle-branch selection to `harpoon-core` (new module or
  extension of `pipe_api.rs`): inputs = visibility state (event-derived),
  own-pane tab position (Option), active tab position (Option, None on cold
  spawn); output = branch enum {Hide, ShowInPlace, ShowThenRelocate{target},
  ColdShow}. Encode the mandatory un-suppress-before-relocate ordering in the
  output contract. Native tests cover all four branches plus the
  cold-spawn/no-cached-state case (AC
  `pane-pipe-api.toggle-pipe-invocation`).
  - intent: feature
  - files_allowed:
      - harpoon-core/src/**
- [x] 2.2 Run `cargo test` (native, harpoon-core) — all green.

## 3. Plugin shim wiring

- [x] 3.1 Establish toggle state via synchronous host queries at pipe time
  (AC `pane-pipe-api.toggle-state-sync-query-verified`):
  `get_pane_info(PaneId::Plugin(own))` for suppressed/visible state,
  `get_focused_pane_info()` → `get_tab_info(tab_id)` for the invoking tab's
  position. Never read cached `TabUpdate`/`PaneUpdate` for toggle decisions
  (probe evidence: caches freeze while suppressed) and never rely on
  `Event::Visible` (probe evidence: only emitted to tiled plugin panes —
  amended from the original event-derived design per task 1.1/1.2 findings).
  - intent: feature
  - files_allowed:
      - harpoon-plugin/src/main.rs
- [x] 3.2 Handle the `toggle` pipe name: feed core branch selection, execute
  the selected branch via host calls (`hide_self`, `show_self(true)`,
  `break_panes_to_tab_with_index`), honoring the R3 decision from task 1.2.
  CLI-sourced `toggle` pipes follow the existing Cli Pipe Client Release
  exactly-once unblock discipline.
  - intent: feature
  - files_allowed:
      - harpoon-plugin/src/main.rs
      - harpoon-core/src/**
- [x] 3.3 Build for the only supported target (Constitution III):
  `cargo build --target wasm32-wasip1 --release` — clean compile.

## 4. Regression scenarios (validation source) + R1 evidence

- [x] 4.1 Author `scripts/toggle-pipe-regression.sh` (tmux-hosted harness;
  precedent `scripts/fullscreen-regression.sh` and
  `scripts/cli-pipe-permission-regression.sh`) asserting, against a freshly
  built harpoon.wasm with a `MessagePlugin`-style toggle invocation: (a)
  same-tab re-invoke after Esc-close shows menu+view on the invoking tab;
  (b) cross-tab invoke AFTER a tab close (forcing tab-id/position drift)
  lands menu+view on the invoking tab; (c) visible→toggle hides. Cite AC
  `pane-pipe-api.toggle-pipe-invocation` in the script header.
  - intent: feature
  - files_allowed:
      - scripts/toggle-pipe-regression.sh
- [x] 4.2 Run `scripts/toggle-pipe-regression.sh`; record the pass AND the
  R1 flicker observation (cross-tab show-then-relocate visual artifact:
  none / single-frame / worse — escalate if worse) in review.md Execution
  Notes. This is the agent-independent validation source
  (`validation_source_mode: required`).

## 5. Runtime activation documentation (operational — outside gate assertions)

- [x] 5.1 Document in README: deploy wasm to
  `~/.config/zellij/plugins/harpoon.wasm`; replace the chezmoi-managed
  `~/.config/zellij/config.kdl` `Ctrl y` binding (`LaunchOrFocusPlugin`
  block → `MessagePlugin` with `name "toggle"` and the same plugin URL);
  reload zellij config (or restart the server); verify a warm toggle
  round-trip (show → Esc → re-invoke from another tab). Note explicitly:
  until the keybind is swapped, invocation still routes through the broken
  `focus_plugin_pane` path.
  - intent: feature
  - files_allowed:
      - README.md
      - openspec/changes/pipe-toggle-invocation/**
