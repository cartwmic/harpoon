<!-- authored: in-session -->
## 1. Implementation

- [x] 1.1 `pipe()` handler: unblock CLI pipe input exactly once after handling
      every `PipeSource::Cli` message (all arms incl. unrecognized-name
      no-op); non-CLI sources untouched. Cite AC
      `pane-pipe-api.cli-pipe-client-release` at the call site.
  - intent: fix
  - files_allowed:
      - harpoon-plugin/src/main.rs
  - allow_new_files: false

## 2. Regression guard

- [x] 2.1 `scripts/fullscreen-regression.sh`: assert exit code 0 for every
      `zellij pipe` invocation (client released within timeout); record a
      fresh full-harness run's evidence below.
  - intent: fix
  - files_allowed:
      - scripts/fullscreen-regression.sh
  - allow_new_files: false

## 3. Verification

- [x] 3.1 Gates green in worktree: `openspec validate --changes --strict`,
      wasm32-wasip1 release build, `cargo test -p harpoon-core`.
  - intent: fix
  - files_allowed:
      - openspec/changes/unblock-cli-pipe-clients/**
  - allow_new_files: false

## Evidence

- 2026-07-10 — scripts/fullscreen-regression.sh, two consecutive full runs at
  the unblock build: 11/11 assertions PASS each (S1–S3 now include
  pipe-client-released exit-0 assertions; S4 visibility polling hardened
  with settle() after two load-induced timing flakes, then green twice).
- cargo test -p harpoon-core: 238 pass; wasm32-wasip1 release build green.
