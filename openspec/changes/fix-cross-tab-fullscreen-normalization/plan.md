<!-- authored: in-session -->
# Execution Plan

## Plan step 1: SDK bump

- **Covers:** T1.1
- **Pre-conditions:**
  - Worktree `opsx/fix-cross-tab-fullscreen-normalization` created; locator
    captured in review.md.
- **Action:**
  1. Edit workspace `Cargo.toml`: `zellij-tile = "0.44.3"`.
  2. `cargo update -p zellij-tile` (accept transitive churn; lockfile-only).
  3. Fix compile errors — expected only `focus_terminal_pane(id, true, false)`
     call sites; investigate anything beyond that before proceeding.
  4. `cargo build --release -p harpoon --target wasm32-wasip1` → green.
  5. `cargo test -p harpoon-core` → green (235 tests baseline).
  6. Commit `build: bump zellij-tile 0.42.2 -> 0.44.3` (behavior-neutral).
- **Verification:** wasm build + core tests, zero behavior diffs.
- **Rollback:** revert the single commit; lockfile restores with it.

## Plan step 2: core decision function

- **Covers:** T2.1
- **Pre-conditions:** step 1 committed.
- **Action:**
  1. Add pure fn in `harpoon-core` (e.g. `jump_toggle_plan(tab_is_fullscreen:
     bool, target: u32) -> Vec<Effect>`): fullscreen-tab ⇒ no enter-toggle
     needed post-swap / tiled ⇒ enter-toggle after focus — encode exactly the
     ground-truth contract, no cached inputs in the signature.
  2. Native tests for all quadrants; each test cites its AC ID literal
     (`pane-pipe-api.jump-to-pane-by-id`,
     `pane-pipe-api.ground-truth-fullscreen-normalization`).
  3. Commit.
- **Verification:** `cargo test -p harpoon-core`; AC ID literals grep-able.
- **Rollback:** revert commit; shim untouched so plugin still builds.

## Plan step 3: shim rewrite + persistence revert

- **Covers:** T2.2, T2.3
- **Pre-conditions:** step 2 committed.
- **Action:**
  1. `jump_focus_fullscreen`: focus target → synchronous post-focus host query
     for the tab's actual fullscreen state (0.44.3 API; spike-proven) → apply
     core `jump_toggle_plan`. Delete `in_active` gate, cached
     `tab_is_fullscreen`/`pane_in_active_tab` decision uses, and the
     `PaneInfo.is_fullscreen` guard. IF the post-focus query cannot be made
     synchronous/reliable: STOP — do not substitute a predictive fallback
     (intent Non-goals); surface to the owner.
  2. `close_helper`: `hide_self()` restores instance persistence; rewrite the
     d6a2039 rationale comment (quirk dead on 0.44.3, spec mandate rejoined).
     Keep `[Effect::Close, Effect::FocusPane]` ordering (Constitution V).
  3. Build wasm; run core tests; commit.
- **Verification:** wasm build green; core tests green; no remaining decision
  reads of cached fullscreen state (`grep -n "is_fullscreen" main.rs` reviewed).
- **Rollback:** revert commit(s); step-1/2 commits stand alone.

## Plan step 4: regression harness

- **Covers:** T3.1, T3.2
- **Pre-conditions:** step 3 committed; local zellij 0.44.3 + tmux available.
- **Action:**
  1. Author `scripts/fullscreen-regression.sh` from the 2026-07-09 exploration
     harness: tmux-hosted isolated zellij session, permission seeding with
     backup/restore, scenario drivers + screen/log assertions for scenarios
     1–4, cleanup on exit.
  2. Run against the freshly built wasm; all four scenarios must pass.
  3. Record per-scenario evidence lines in review.md Execution Notes; check
     off T3.2.
- **Verification:** script exits 0 with 4/4 scenario PASS lines.
- **Rollback:** script is additive; delete on failure. A scenario failure is
  a defect in steps 2–3, not in the harness — fix there and re-run.

## Plan step 5: docs

- **Covers:** T4.1
- **Action:** README pipe examples targeted (`--plugin`, note
  `--plugin-configuration` matching); broadcast hazard paragraph; 0.44.3
  runtime floor. Commit.
- **Verification:** README grep: no `zellij pipe --name jump_pane` example
  without `--plugin`.
- **Rollback:** revert commit.

## Plan step 6: verification wrap

- **Covers:** T5.1
- **Action:** run all three gates in the worktree; confirm AC-citing tests
  exist for the three delta AC IDs; check off tasks.
- **Verification:** `opsx gate fix-cross-tab-fullscreen-normalization
  --worktree <path>` reaches the review stage (no artifact/validation reds).
- **Rollback:** n/a (read-only checks).

## Completion Verification

- `openspec validate --changes --strict` → pass
- `cargo build --release -p harpoon --target wasm32-wasip1` → success
- `cargo test -p harpoon-core` → all pass
- `scripts/fullscreen-regression.sh` → 4/4 scenarios PASS
- `grep -rn "pane-pipe-api.jump-to-pane-by-id\|pane-pipe-api.ground-truth-fullscreen-normalization" harpoon-core/` → ≥1 test hit per AC

## Manual Adjustments

- Execution Mode standard (not tdd-required): host-side fullscreen behavior is
  untestable natively; TDD applies only to the core decision function (step 2
  is written test-first anyway).
