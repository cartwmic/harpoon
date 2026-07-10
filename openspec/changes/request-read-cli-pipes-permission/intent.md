# Intent — request-read-cli-pipes-permission

**Status:** FROZEN (explore concluded 2026-07-10)
**Recommended Scale:** S, `full_rigor: false`

## Intent

The harpoon plugin has never requested `PermissionType::ReadCliPipes`
(`harpoon-plugin/src/main.rs` `load()` requests only `RunCommands`,
`ReadApplicationState`, `ChangeApplicationState`; `git log -S ReadCliPipes`
shows it was never requested in any commit). Zellij gates two host calls
behind that permission — `unblock_cli_pipe_input` and `cli_pipe_output` —
so both are silently permission-denied at runtime (190+ denial lines in the
zellij server log, 2026-07-05 → 2026-07-10). Consequence: the Cli Pipe
Client Release requirement shipped by commit `23ade2a`
(2026-07-10-unblock-cli-pipe-clients) is a deployed no-op — its entire
mechanism is the denied `unblock_cli_pipe_input` call — and every
CLI-sourced `jump_pane` pipe strands a zombie `zellij pipe` client (plus,
in the ntfy-tap chain, a hung ssh side-channel session that eventually
exhausts sshd `MaxSessions` and kills notification jumps entirely).
`slot_for_pane` output has been silently broken since introduction for the
same reason.

Add `PermissionType::ReadCliPipes` to the `request_permission` call so the
already-specified client-release behavior actually holds at runtime and
`slot_for_pane` can produce output.

## Constraints

- Code delta is confined to the `request_permission` call in
  `harpoon-plugin/src/main.rs` `load()` (plus `PermissionRequestResult`
  handling only if verification demands it). No behavior changes elsewhere.
- Spec delta required in `openspec/specs/pane-pipe-api/spec.md`: a
  requirement that the plugin SHALL request every `PermissionType` its
  host calls require (`ReadCliPipes` for `unblock_cli_pipe_input` /
  `cli_pipe_output`), with a scenario asserting those calls are not
  permission-denied after grant.
- Regression evidence: a committed scenario (precedent:
  `scripts/fullscreen-regression.sh` tmux-hosted harness) asserting a CLI
  `jump_pane` pipe client exits promptly (no hang, no `timeout 124`) and
  the server log gains no `ReadCliPipes' denied` lines. Native
  `harpoon-core` tests cannot cover host permission behavior; a scenario
  script is the acceptable vehicle.
- Build target wasm32-wasip1 only (Constitution III).
- Runtime activation is operational and OUTSIDE gate assertions, but MUST
  be documented in the change (tasks or README): deploy wasm to
  `~/.config/zellij/plugins/harpoon.wasm`; answer the new permission
  prompt in a VISIBLE plugin pane; verify `permissions.kdl` gains
  `ReadCliPipes` for that path; restart the long-lived `workspace` zellij
  server to clear the accumulated pipe wedge. Skipping the visible-pane
  regrant leaves the fix inert.
- Sequencing: `fix-cross-tab-fullscreen-normalization` is gate-passed at
  loop_hold (integration commit `d930ab1`, worktree HEAD `1303ceef`)
  awaiting archive authorization. This change bases on `main` and MUST
  rebase cleanly over that archive if it lands first. No scope
  interleaving between the two changes.

## Invariants honored

- Constitution II (Specs are the source of behavior): permission
  completeness lands as a pane-pipe-api spec requirement alongside the
  code, not as code-only.
- Constitution III (wasm32-wasip1 is the only build target).
- Constitution IV (Never act on unverified host state): grant is verified
  (permissions.kdl / PermissionRequestResult), not assumed.
- Constitution V (Canonical effect ordering): untouched — no change to
  jump/fullscreen effect ordering.
- pane-pipe-api "Cli Pipe Client Release" and "Targeted Pipe Delivery"
  requirements remain as specified; this change makes the former
  enforceable at runtime.

## Non-goals

- Session routing (`-s`/`--session` on pipes) or multi-session support.
- Phone-side `termux/zellij-jump` hardening (timeout/keepalive,
  `--plugin-configuration` warm-instance targeting) — separate chezmoi
  change `harden-zellij-jump-transport`.
- sshd `MaxSessions` or any ssh/ControlMaster configuration.
- Automating the zellij server restart.
- termux-app fork changes.
- Any expansion of the `slot_for_pane` feature beyond it starting to work.
