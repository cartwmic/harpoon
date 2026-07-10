## 1. Permission request fix

- [x] 1.1 Add `PermissionType::ReadCliPipes` to the `request_permission` call
  in `load()` (`harpoon-plugin/src/main.rs`), keeping the existing three
  permissions. Extend `PermissionRequestResult` handling ONLY if verification
  (task 2.x) demands it.
  - intent: fix
  - files_allowed:
      - harpoon-plugin/src/main.rs
  - allow_new_files: false
- [x] 1.2 Build for the only supported target (Constitution III) and confirm
  clean compile: `cargo build --target wasm32-wasip1 --release`.

## 2. Regression scenario (validation source)

- [ ] 2.1 Author `scripts/cli-pipe-permission-regression.sh` (tmux-hosted
  harness; precedent `scripts/fullscreen-regression.sh`) asserting, against a
  freshly built harpoon.wasm in a scripted zellij session with the permission
  granted: (a) a CLI `jump_pane` pipe client exits promptly — no hang, no
  `timeout` exit 124; (b) the zellij server log gains no `ReadCliPipes'
  denied` lines. Cite AC `pane-pipe-api.host-call-permission-completeness` in
  the script header.
  - intent: feature
  - files_allowed:
      - scripts/cli-pipe-permission-regression.sh
- [ ] 2.2 Run `scripts/cli-pipe-permission-regression.sh`; record the pass in
  the change's Execution Notes. This is the agent-independent validation
  source (`validation_source_mode: required`); native `harpoon-core` tests
  cannot cover host permission behavior.

## 3. Runtime activation documentation (operational — outside gate assertions)

- [ ] 3.1 Document the runtime activation steps in the README (or this
  change's docs): deploy wasm to `~/.config/zellij/plugins/harpoon.wasm`;
  answer the new permission prompt in a VISIBLE plugin pane; verify
  `~/.config/zellij/permissions.kdl` gains `ReadCliPipes` for that plugin
  path; restart the long-lived `workspace` zellij server to clear the
  accumulated pipe wedge. Note explicitly: skipping the visible-pane regrant
  leaves the fix inert.
  - intent: feature
  - files_allowed:
      - README.md
      - openspec/changes/request-read-cli-pipes-permission/**
