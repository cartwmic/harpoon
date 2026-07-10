<!-- authored: in-session -->
## Why

Every phone ntfy tap leaves a zombie `zellij pipe --name jump_pane` client
process (23+ accumulated Jul 8–9; likely hung ssh sessions phone-side too).
Live diagnosis on the long-lived `workspace` server showed the host's implicit
CLI-pipe client release is racy (identical back-to-back pipes: exit 0, then
hang), while fresh servers always release. harpoon's `pipe()` handler never
calls `unblock_cli_pipe_input`, so client termination depends entirely on that
racy implicit path. Frozen baseline: intent.md.

## What Changes

- `pipe()` handler (harpoon-plugin/src/main.rs): for `PipeSource::Cli`
  messages, call `unblock_cli_pipe_input(&pipe_message.name)` exactly once
  after handling — all arms, including the unrecognized-name no-op.
- `scripts/fullscreen-regression.sh`: assert every `zellij pipe` invocation
  exits 0 within its timeout (client released), so the release contract is
  regression-guarded.

## Capabilities

### New Capabilities

- (none)

### Modified Capabilities

- `pane-pipe-api`: ADDED requirement — CLI pipe client release.

## Impact

**Affected files**

- `harpoon-plugin/src/main.rs` (pipe handler only)
- `scripts/fullscreen-regression.sh` (exit-code assertions)
- `openspec/specs/pane-pipe-api/spec.md` (delta in this change)
