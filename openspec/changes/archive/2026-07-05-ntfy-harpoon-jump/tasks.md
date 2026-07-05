## 1. Core: pane-id parsing + slot reverse lookup (pure, harpoon-core)

- [x] 1.1 Add a pure pane-id parser: `terminal_N` | bare `N` → `Option<u32>`
    (rejects `plugin_N`, empty, non-numeric). Cite AC
    `pane-pipe-api.pane-id-string-parsing`.
  - intent: feature
  - files_allowed:
      - harpoon-core/src/**/*.rs
  - allow_new_files: true
- [x] 1.2 Add a pure `slot_for_pane` lookup over `BookmarkStore`
    (`pane_id_to_bookmark_idx`) returning the 1-based slot or `None`; MUST NOT
    mutate the store. Cite AC `pane-pipe-api.slot-for-pane-reverse-lookup`.
  - intent: feature
  - files_allowed:
      - harpoon-core/src/**/*.rs
  - allow_new_files: false

## 2. Plugin: pipe() handler wiring (harpoon-plugin)

- [x] 2.1 Implement `ZellijPlugin::pipe()` on `State`: match message name
    `slot_for_pane` → parse pane id, run core lookup, answer via
    `cli_pipe_output` (empty string when absent). Cite AC
    `pane-pipe-api.slot-for-pane-reverse-lookup`.
  - intent: feature
  - files_allowed:
      - harpoon-plugin/src/**/*.rs
  - allow_new_files: false
- [x] 2.2 In `pipe()`, match message name `jump_pane` → parse pane id, call the
    existing `jump_focus_fullscreen(id)`; no-op on unresolvable payload. Cite AC
    `pane-pipe-api.jump-to-pane-by-id`.
  - intent: feature
  - files_allowed:
      - harpoon-plugin/src/**/*.rs
  - allow_new_files: false

## 3. Tests (harpoon-core)

- [x] 3.1 Unit tests for the pane-id parser: `terminal_7`→7, `7`→7,
    `plugin_3`/``/`abc`→None. Cite AC `pane-pipe-api.pane-id-string-parsing`.
  - intent: feature
  - files_allowed:
      - harpoon-core/src/**/*.rs
      - harpoon-core/tests/**/*.rs
  - allow_new_files: true
- [x] 3.2 Unit tests for `slot_for_pane`: harpooned id → Some(1-based), absent id
    → None, and store-unchanged assertion. Cite AC
    `pane-pipe-api.slot-for-pane-reverse-lookup`.
  - intent: feature
  - files_allowed:
      - harpoon-core/src/**/*.rs
      - harpoon-core/tests/**/*.rs
  - allow_new_files: true
