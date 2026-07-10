# Capability: pane-pipe-api

## ADDED Requirements

### Requirement: Host Call Permission Completeness

<!-- AC ID: pane-pipe-api.host-call-permission-completeness -->

THE plugin SHALL request, at load, every `PermissionType` required by a host
call it invokes — including `ReadCliPipes`, which zellij requires for
`unblock_cli_pipe_input` and `cli_pipe_output` — so that no host call the
plugin depends on is silently permission-denied at runtime.

#### Scenario: ReadCliPipes requested at load
- **WHEN** the plugin's `load()` runs
- **THEN** the permission request SHALL include `ReadCliPipes` alongside the
  existing `RunCommands`, `ReadApplicationState`, and
  `ChangeApplicationState` permissions

#### Scenario: pipe host calls not denied after grant
- **WHILE** `ReadCliPipes` has been granted
- **WHEN** a CLI-sourced pipe message (`jump_pane`, `slot_for_pane`, or an
  unrecognized name) is handled
- **THEN** the resulting `unblock_cli_pipe_input` / `cli_pipe_output` host
  calls SHALL NOT be permission-denied (the zellij server log SHALL gain no
  `ReadCliPipes' denied` lines)
- **AND** the invoking `zellij pipe` client process SHALL exit promptly
  (no hang, no `timeout` exit 124)

#### Scenario: ungranted permission is not assumed
- **IF** the host reports the permission request denied
  (`PermissionRequestResult`)
- **THEN** the plugin SHALL NOT treat the gated pipe-release and pipe-output
  behavior as available (grant is verified, never assumed — Constitution IV)

## MODIFIED Requirements

## REMOVED Requirements

## RENAMED Requirements

---

## Acceptance criterion quality checklist

| AC ID | Testable | Solution-free | Unambiguous | Consistent | Complete |
|---|---|---|---|---|---|
| pane-pipe-api.host-call-permission-completeness | [x] | [x] | [x] | [x] | [x] |
