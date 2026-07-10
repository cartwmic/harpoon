<!-- authored: in-session -->
## 1. SDK bump (zellij-tile 0.42.2 → 0.44.3)

- [ ] 1.1 Bump workspace `zellij-tile` to `0.44.3`; refresh Cargo.lock; fix
      call-site signature changes (known: `focus_terminal_pane(id, true, false)`
      gains a 3rd bool). Build + tests must pass unchanged in behavior.
  - intent: infra
  - files_allowed:
      - Cargo.toml
      - Cargo.lock
      - harpoon-plugin/**/*.rs
      - harpoon-core/**/*.rs
  - allow_new_files: false

## 2. Ground-truth fullscreen normalization

- [ ] 2.1 Move the toggle decision into `harpoon-core` (Constitution I): a pure
      decision function that takes ground-truth tab-fullscreen state + target
      pane id and returns the toggle/focus effect sequence. Native tests cite
      AC IDs `pane-pipe-api.jump-to-pane-by-id` and
      `pane-pipe-api.ground-truth-fullscreen-normalization`.
  - intent: feature
  - files_allowed:
      - harpoon-core/**/*.rs
  - allow_new_files: true
- [ ] 2.2 Rewrite `jump_focus_fullscreen` in the shim as: focus target →
      synchronous post-focus ground-truth query of the tab's fullscreen state
      (0.44.3 host API; verified viable in the 2026-07-09 exploration spike) →
      toggle only when provably tiled, via the core decision function. Delete
      the `in_active`/cross-tab gate and every decision use of cached
      `TabInfo`/`PaneInfo.is_fullscreen`; update the stale
      "plugin-closed-on-jump" design comments.
  - intent: fix
  - files_allowed:
      - harpoon-plugin/src/main.rs
      - harpoon-core/**/*.rs
  - allow_new_files: false
- [ ] 2.3 Revert d6a2039: `close_helper` returns to `hide_self()`; replace the
      close_self rationale comment with the 0.44.3 persistence rationale
      (quirk dead; spec mandate rejoined). Preserve `[Effect::Close,
      Effect::FocusPane]` ordering.
  - intent: fix
  - files_allowed:
      - harpoon-plugin/src/main.rs
  - allow_new_files: false

## 3. Regression harness (committed, repeatable)

- [ ] 3.1 Commit the tmux-hosted isolated-zellij spike harness as
      `scripts/fullscreen-regression.sh`: builds the wasm, seeds plugin
      permissions (with backup/restore), and drives the four mandatory
      scenarios from intent.md — (1) cold-start pipe → hidden pane of
      fullscreen tab; (2) cold-start pipe → the fullscreened pane itself;
      (3) warm cross-tab jump; (4) hide→relaunch cycle under a fullscreen
      terminal pane (mis-focus detector, no new `load()`).
  - intent: infra
  - files_allowed:
      - scripts/**
  - allow_new_files: true
- [ ] 3.2 Run the harness against the built wasm; record pass/fail evidence
      per scenario in review.md Execution Notes (evidence lines cite the
      scenario numbers).
  - intent: fix
  - files_allowed:
      - openspec/changes/fix-cross-tab-fullscreen-normalization/tasks.md
  - allow_new_files: false

## 4. Documentation

- [ ] 4.1 README: pipe invocation examples use explicit `--plugin` (and note
      `--plugin-configuration` instance-matching); document the broadcast
      double-toggle hazard (AC `pane-pipe-api.targeted-pipe-delivery`); state
      the zellij 0.44.3 runtime floor.
  - intent: feature
  - files_allowed:
      - README.md
  - allow_new_files: false

## 5. Verification

- [ ] 5.1 All gates green in the worktree: `openspec validate --changes
      --strict`, `cargo build --release -p harpoon --target wasm32-wasip1`,
      `cargo test -p harpoon-core`. AC-citing tests present for the three
      delta AC IDs (grep-verifiable literals).
  - intent: fix
  - files_allowed:
      - harpoon-core/**/*.rs
      - openspec/changes/fix-cross-tab-fullscreen-normalization/**
  - allow_new_files: false
