**Change:** ntfy-harpoon-jump
**Doneness:** satisfied
**Judge:** worker/claude-bridge/claude-opus-4-8
**review_mode:** blind-single-judge
**Frozen-Intent SHA:** 9a1fa8690a7964b5b1f46cbe7d16b943a7506981cc39bca5b40ce433aa425221
**Diff Base SHA:** 80627802fc3e6eeda338f6501b861ce5aea22c4c
**Reviewed Range:** 80627802fc3e6eeda338f6501b861ce5aea22c4c..bf355f69252b721686956177e2b086969456948b
**Generated:** 2026-07-04

The frozen intent's stated outcomes are all met:

1. **Both primitives exist.** `parse_pane_id` + `slot_for_pane` land as pure,
   natively-tested helpers in `harpoon-core/src/pipe_api.rs` and are re-exported
   from `harpoon-core/src/lib.rs`.
2. **Wired to the pipe handler.** `harpoon-plugin/src/main.rs` implements
   `ZellijPlugin::pipe()`, matching `PipeSource::Cli` and dispatching
   `slot_for_pane` → `cli_pipe_output` (empty string on miss) and `jump_pane` →
   existing `jump_focus_fullscreen(id)`; unresolvable payloads are no-ops.
3. **Tested.** `cargo test -p harpoon-core` → 235 passed (≥ 223 baseline),
   covering both forms of pane-id parsing, rejection of `plugin_N`/empty/
   non-numeric/overflow, 1-based reverse lookup, absent + unmaterialized cases,
   and a store-unchanged (pure-read) assertion.
4. **Constraints honored.** wasm32-wasip1 release build compiles clean; the
   superseded `PaneInfo.is_fullscreen` heuristic is not reintroduced; no pipe
   permissions were broadened.

No unmet outcomes; no gold-plating demanded.
