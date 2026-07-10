# Intent: unblock-cli-pipe-clients

Scale: S
Frozen: 2026-07-09 (post-deploy live diagnosis of fix-cross-tab-fullscreen-normalization)

## Intent

Every CLI-sourced pipe message harpoon receives (`jump_pane`, `slot_for_pane`,
and any unrecognized name delivered to the plugin) MUST end with the CLI pipe
input explicitly unblocked (`unblock_cli_pipe_input`), so the `zellij pipe`
client process terminates deterministically instead of relying on the host's
implicit release. Live diagnosis (2026-07-09, month-old `workspace` server)
showed the implicit release is racy on long-lived servers: 23+ zombie
`zellij pipe` clients accumulated over Jul 8–9 (one per ntfy tap), while the
jump itself executed correctly; back-to-back identical pipes released (exit 0)
then hung (exit 124) seconds apart. Explicit unblock removes the race and the
phone-side hung-ssh tail the zombies imply.

## Constraints

- Shim-only change (`harpoon-plugin/src/main.rs` `pipe()` handler) plus spec
  delta and harness assertion; no normalization/jump behavior change.
- Unblock fires for `PipeSource::Cli` messages only, exactly once per message,
  AFTER handling (including the unrecognized-name no-op arm) — never for
  plugin-to-plugin pipes.
- Regression harness gains client-exit assertions: every `zellij pipe`
  invocation in `scripts/fullscreen-regression.sh` must exit 0 within its
  timeout (no lingering client).
- Gates stay green: `openspec validate --changes --strict`, wasm32-wasip1
  build, `cargo test -p harpoon-core`.

## Invariants honored

- pane-pipe-api: parsing, jump outcome, and targeted-delivery requirements
  unchanged; this adds a client-release requirement alongside them.
- jump-mode "read-only with respect to harpoon state": unblock touches no
  slot state.
- Constitution I: no decision logic added to the shim (unblock is a
  transport-level host call, not a decision).

## Non-goals

- No investigation/remediation of the degraded month-old server's plugin-load
  wedge (restart-only condition; out of scope).
- No changes to notification chain repos.
- No blanket unblock of pipes harpoon never receives.
