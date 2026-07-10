# Proposal — request-read-cli-pipes-permission

## Why

The plugin never requests `PermissionType::ReadCliPipes`, so zellij
permission-denies `unblock_cli_pipe_input` and `cli_pipe_output` at runtime
(190+ denial lines in the server log, 2026-07-05 → 2026-07-10). The shipped
Cli Pipe Client Release requirement (commit `23ade2a`) is a deployed no-op:
every CLI-sourced `jump_pane` strands a zombie `zellij pipe` client, and
`slot_for_pane` output has been silently broken since introduction.
Constitution IV (never act on unverified host state) drives the verified-grant
constraint; Constitution II drives landing this as a spec requirement, not
code-only.

## What Changes

- Add `PermissionType::ReadCliPipes` to the `request_permission` call in
  `harpoon-plugin/src/main.rs` `load()` (currently requests only
  `RunCommands`, `ReadApplicationState`, `ChangeApplicationState`).
- Add a `pane-pipe-api` spec requirement: the plugin SHALL request every
  `PermissionType` its host calls require (`ReadCliPipes` for
  `unblock_cli_pipe_input` / `cli_pipe_output`), with a scenario asserting
  those calls are not permission-denied after grant.
- Add a committed regression scenario script (precedent:
  `scripts/fullscreen-regression.sh` tmux-hosted harness) asserting a CLI
  `jump_pane` pipe client exits promptly (no hang, no `timeout` exit 124) and
  the server log gains no `ReadCliPipes' denied` lines.
- Document runtime activation steps (deploy wasm, answer the new permission
  prompt in a VISIBLE plugin pane, verify `permissions.kdl` gains
  `ReadCliPipes`, restart the long-lived `workspace` zellij server) — these
  are operational, outside gate assertions.

No breaking changes.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pane-pipe-api`: new requirement — permission completeness for host calls
  (`ReadCliPipes` requested at load so `unblock_cli_pipe_input` /
  `cli_pipe_output` are not permission-denied). Existing "Cli Pipe Client
  Release" and "Targeted Pipe Delivery" requirements unchanged; this makes
  the former enforceable at runtime.

## Impact

- **Affected files:**
  - `harpoon-plugin/src/main.rs` — `request_permission` call in `load()`
    (plus `PermissionRequestResult` handling only if verification demands it)
  - `openspec/specs/pane-pipe-api/spec.md` — delta requirement (via change
    `specs/pane-pipe-api/spec.md`)
  - `scripts/` — new committed regression scenario script
  - README or tasks — runtime activation documentation
- **Build:** wasm32-wasip1 only (Constitution III). Native `harpoon-core`
  tests cannot cover host permission behavior; the scenario script is the
  validation vehicle.
- **Sequencing:** bases on `main`; MUST rebase cleanly over the
  `fix-cross-tab-fullscreen-normalization` archive if that lands first. No
  scope interleaving.
