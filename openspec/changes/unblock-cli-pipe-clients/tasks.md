<!-- authored: in-session -->
## 1. Implementation

- [ ] 1.1 `pipe()` handler: unblock CLI pipe input exactly once after handling
      every `PipeSource::Cli` message (all arms incl. unrecognized-name
      no-op); non-CLI sources untouched. Cite AC
      `pane-pipe-api.cli-pipe-client-release` at the call site.
  - intent: fix
  - files_allowed:
      - harpoon-plugin/src/main.rs
  - allow_new_files: false

## 2. Regression guard

- [ ] 2.1 `scripts/fullscreen-regression.sh`: assert exit code 0 for every
      `zellij pipe` invocation (client released within timeout); record a
      fresh full-harness run's evidence below.
  - intent: fix
  - files_allowed:
      - scripts/fullscreen-regression.sh
  - allow_new_files: false

## 3. Verification

- [ ] 3.1 Gates green in worktree: `openspec validate --changes --strict`,
      wasm32-wasip1 release build, `cargo test -p harpoon-core`.
  - intent: fix
  - files_allowed:
      - openspec/changes/unblock-cli-pipe-clients/**
  - allow_new_files: false
