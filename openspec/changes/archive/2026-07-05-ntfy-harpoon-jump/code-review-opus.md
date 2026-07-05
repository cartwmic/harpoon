**Change:** ntfy-harpoon-jump
**Verdict:** pass
**review_mode:** blind-single-model
**reviewer-provenance:** worker/claude-bridge/claude-opus-4-8
**Diff Base SHA:** 80627802fc3e6eeda338f6501b861ce5aea22c4c
**Reviewed Range:** 80627802fc3e6eeda338f6501b861ce5aea22c4c..bf355f69252b721686956177e2b086969456948b
**Generated:** 2026-07-04

## Findings

No P0 or P1 findings. Verdict: **pass**.

The diff adds the two `pane-pipe-api` primitives exactly as the frozen intent and
delta ACs require. Each frozen-baseline element was checked against the code:

- **`slot_for_pane` is a pure read** — `harpoon-core/src/pipe_api.rs:42` reads
  `store.pane_id_to_bookmark_idx` / `store.bookmarks` and returns
  `bookmark.index + 1`; no mutation. The plugin handler
  (`harpoon-plugin/src/main.rs`) calls it via `&self.store` and writes the result
  (or empty string) with `cli_pipe_output`. Non-mutation is asserted by the
  `lookup_does_not_mutate_store` test. Satisfies AC
  `pane-pipe-api.slot-for-pane-reverse-lookup` (1-based: slot index 2 → `3`, and
  absent / `None`-index → empty string).
- **`jump_pane` reuses `jump_focus_fullscreen`** — the handler calls
  `self.jump_focus_fullscreen(id)` (defined at `harpoon-plugin/src/main.rs:521`),
  which normalizes via the authoritative `TabInfo.is_fullscreen_active` two-phase
  path. The superseded `PaneInfo.is_fullscreen` tab-level heuristic is NOT
  reintroduced (`is_fullscreen` appears only as a best-effort cross-tab
  idempotency guard, consistent with commit 6e88511). Satisfies AC
  `pane-pipe-api.jump-to-pane-by-id`.
- **Pane-id parsing reconciles both forms and rejects non-terminal kinds** —
  `parse_pane_id` (`harpoon-core/src/pipe_api.rs:21`) strips a single
  `terminal_` prefix, requires an all-ASCII-digit tail, and `parse::<u32>()`s it.
  `terminal_7`→7, `7`→7, and `plugin_3`/`abc`/``/`terminal_`/`4294967296`→`None`.
  Satisfies AC `pane-pipe-api.pane-id-string-parsing`; unresolvable payloads are
  no-ops in `pipe()` (no state mutation, no focus change).
- **Build target** — `cargo build --release -p harpoon --target wasm32-wasip1`
  compiles clean.
- **Test suite** — `cargo test -p harpoon-core` reports 235 passed (≥ 223
  baseline; +12 new for parser + reverse lookup). No regressions.
- **Pipe permission surface** — the handler only matches `PipeSource::Cli`; the
  plugin's `request_permission` set (RunCommands / ReadApplicationState /
  ChangeApplicationState) is untouched by the diff. No broadened permissions.

### Advisory (P3, non-gating)

- `harpoon-core/src/pipe_api.rs:44` — `bookmark.index.map(|i| i + 1)` on a `u16`
  would overflow at `i == u16::MAX` (65535). Purely theoretical: harpoon slot
  counts are single-digit, so `i + 1` is never near the boundary. No action
  required; noted only for completeness.
