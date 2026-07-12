# Execution Plan

<!-- authored: in-session -->

## Plan step 1: Risk probes R2 + R3

- **Covers:** T1.1, T1.2
- **Pre-conditions:**
  - Worktree exists; `cargo build --target wasm32-wasip1 --release` succeeds
    at Diff Base (pre-change baseline builds).
  - tmux + zellij 0.44.3 available (same harness environment as
    `scripts/fullscreen-regression.sh`).
- **Action:**
  1. Author `scripts/toggle-pipe-probe.sh`: scripted zellij session loading
     the built wasm; a temporary keybind
     `MessagePlugin "file:<wasm>" { name "toggle"; }`; instrumented pipe
     handler logging (temporary eprintln in a throwaway build is acceptable —
     probe evidence, not shipped behavior).
  2. Drive the keybind; capture whether the pipe arrives, its source
     variant, and any permission prompt/denial (R2).
  3. With the plugin hidden via `hide_self()`, log the plugin's own pane
     entry (or absence) in the cached `PaneUpdate` manifest (R3).
  4. Record both outcomes in review.md Execution Notes (integration
     checkout); pick the own-tab detection strategy (manifest lookup vs
     unconditional show-then-relocate fallback).
- **Verification:**
  - Execution Notes contain R2 + R3 evidence lines and the R3 strategy
    decision.
- **Rollback:**
  - Probe script is additive; delete it if abandoned. No production paths
    touched.

## Plan step 2: Core toggle-branch decision logic

- **Covers:** T2.1, T2.2
- **Pre-conditions:** Step 1 decided the own-tab detection strategy (shapes
  the branch-selection inputs).
- **Action:**
  1. Add branch enum + pure selection fn to `harpoon-core` (inputs:
     sync-queried suppressed state, focused-pane identity, parked-tab
     comparison; output: Hide | ShowInPlace | Respawn | ColdShow —
     AS AMENDED by probe evidence + the owner ruling; see review.md Scope
     Expansions).
  2. Native tests: all four branches; cold spawn (no cached state) never
     reads manifest-derived inputs; cross-tab selects Respawn, never a
     guessed in-place show (AC `pane-pipe-api.toggle-pipe-invocation`).
  3. `cargo test` green.
  4. Commit (`feat: core toggle branch selection`).
- **Verification:**
  - `cargo test` (harpoon-core native) green.
- **Rollback:**
  - Additive module; revert the commit.

## Plan step 3: Shim wiring

- **Covers:** T3.1, T3.2, T3.3
- **Pre-conditions:** Step 2 committed.
- **Action:**
  1. Establish toggle state via synchronous host queries at pipe time (AC
     `pane-pipe-api.toggle-state-sync-query-verified` — amended from the
     retired Event::Visible design per probe evidence).
  2. Route the `toggle` pipe name through core branch selection; execute the
     branch via `hide_self` / `show_self(true)` / the owner-ruled respawn
     (`open_plugin_pane_floating` + `close_self`); keep the existing
     CLI-pipe exactly-once unblock discipline for `toggle`.
  3. `cargo build --target wasm32-wasip1 --release` clean (Constitution
     III).
  4. Commit (`feat: toggle pipe show/hide lifecycle`).
- **Verification:**
  - wasm build clean; `cargo test` still green.
- **Rollback:**
  - Revert the commit; existing pipes (`jump_pane`, `slot_for_pane`)
    untouched by design.

## Plan step 4: Regression scenario + R1 evidence

- **Covers:** T4.1, T4.2
- **Pre-conditions:** Step 3 wasm deployed to the scripted session (never
  the live `workspace` server).
- **Action:**
  1. Author `scripts/toggle-pipe-regression.sh` asserting: (a) same-tab
     re-invoke after Esc-close → menu+view on invoking tab; (b) cross-tab
     invoke after a tab close (id/position drift forced) → menu+view on
     invoking tab; (c) visible→toggle hides. AC cited in header.
  2. Run it; capture pass output.
  3. Observe cross-tab relocation for visual artifact (R1): none /
     single-frame acceptable; worse ⇒ STOP and escalate to the user per the
     frozen intent constraint.
  4. Record pass + R1 observation in review.md Execution Notes; check off
     tasks.
- **Verification:**
  - `scripts/toggle-pipe-regression.sh` exits 0 (agent-independent
    validation source).
- **Rollback:**
  - Script is additive. A failing scenario blocks progress (fix code, never
    weaken the scenario).

## Plan step 5: Runtime activation documentation

- **Covers:** T5.1
- **Pre-conditions:** Steps 1-4 done (documented behavior is real).
- **Action:**
  1. README section: deploy wasm; swap the chezmoi-managed `config.kdl`
     `Ctrl y` binding from `LaunchOrFocusPlugin` to `MessagePlugin` with
     `name "toggle"` (same plugin URL); reload config; verify warm
     round-trip. Note: until the keybind swap, invocation still routes
     through the broken `focus_plugin_pane` path.
  2. Commit (`docs: toggle-pipe runtime activation`).
- **Verification:**
  - README renders; steps match shipped behavior.
- **Rollback:**
  - Docs-only commit; revert.

## Completion Verification

- `cargo test` (harpoon-core, native) — green.
- `cargo build --target wasm32-wasip1 --release` — clean.
- `scripts/toggle-pipe-regression.sh` — exit 0 (covers ACs
  `pane-pipe-api.toggle-pipe-invocation`,
  `pane-pipe-api.toggle-state-sync-query-verified`).
- Execution Notes carry R1/R2/R3 evidence (frozen-intent obligation).

## Manual Adjustments

- Execution Mode `standard` (not tdd-required): plan steps use simple
  ordered actions; tests still land with the core logic in step 2 and the
  scenario harness in step 4.
