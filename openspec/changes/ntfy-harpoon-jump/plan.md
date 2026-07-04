# Execution Plan

Execution Mode = standard (not tdd-required), so steps are ordered lists. Tests
are authored alongside the pure core logic they cover.

## Plan step 1: Pure core — pane-id parser

- **Covers:** T1.1, T3.1
- **Pre-conditions:**
  - harpoon-core builds green; 223-test baseline passing.
- **Action:**
  1. Add `parse_pane_id(&str) -> Option<u32>` to harpoon-core (accept
     `terminal_N` and bare `N`; reject `plugin_N`, empty, non-numeric).
  2. Add unit tests covering `terminal_7`→7, `7`→7, `plugin_3`/``/`abc`→None
     (cite `pane-pipe-api.pane-id-string-parsing`).
  3. `cargo test -p harpoon-core` → PASS.
- **Verification:**
  - `cargo test -p harpoon-core`
- **Rollback:**
  - Remove the new fn + tests (isolated; no existing call sites touched).

## Plan step 2: Pure core — slot reverse lookup

- **Covers:** T1.2, T3.2
- **Pre-conditions:**
  - Step 1 landed.
- **Action:**
  1. Add `slot_for_pane(&BookmarkStore, id: u32) -> Option<u16>` (1-based) that
     reads `pane_id_to_bookmark_idx` without mutation.
  2. Add unit tests: harpooned id → `Some(idx+1)`, absent id → `None`, plus a
     store-unchanged assertion (cite `pane-pipe-api.slot-for-pane-reverse-lookup`).
  3. `cargo test -p harpoon-core` → PASS.
- **Verification:**
  - `cargo test -p harpoon-core`
- **Rollback:**
  - Remove the new fn + tests.

## Plan step 3: Plugin pipe() handler

- **Covers:** T2.1, T2.2
- **Pre-conditions:**
  - Steps 1-2 landed (core primitives available).
- **Action:**
  1. Implement `fn pipe(&mut self, pipe_message: PipeMessage) -> bool` on
     `State`, matching `PipeSource::Cli`.
  2. `slot_for_pane` → `parse_pane_id` + core `slot_for_pane` → `cli_pipe_output`
     (empty string when `None`).
  3. `jump_pane` → `parse_pane_id` + existing `self.jump_focus_fullscreen(id)`;
     no-op on unresolvable payload.
  4. `cargo build --release -p harpoon --target wasm32-wasip1` → PASS.
- **Verification:**
  - `cargo build --release -p harpoon --target wasm32-wasip1`
  - `cargo test -p harpoon-core`
- **Rollback:**
  - Remove the `pipe()` method (additive; no existing trait methods changed).

## Completion Verification

- `cargo test -p harpoon-core` — all tests green (≥ 223 baseline + new).
- `cargo build --release -p harpoon --target wasm32-wasip1` — compiles clean.
- `openspec validate --changes --strict` — exit 0.

## Manual Adjustments

- Standard (non-tdd) execution: pure core logic lands with its tests in the same
  step; plugin wiring is thin glue over already-tested primitives, verified by
  the wasm build (host FFI not unit-testable off-target).
