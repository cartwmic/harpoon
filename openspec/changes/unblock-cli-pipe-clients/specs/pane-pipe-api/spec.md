# Capability: pane-pipe-api

## ADDED Requirements

### Requirement: Cli Pipe Client Release

WHEN THE plugin finishes handling a CLI-sourced pipe message (`jump_pane`,
`slot_for_pane`, or an unrecognized name), THE plugin SHALL unblock that CLI
pipe's input exactly once, so the invoking `zellij pipe` client process
terminates without depending on the host's implicit release.

#### Scenario: jump_pane client terminates
- **WHEN** a CLI `jump_pane` pipe message is handled (resolvable or not)
- **THEN** the plugin SHALL unblock the `jump_pane` pipe input after handling
- **AND** the invoking client process SHALL terminate

#### Scenario: unrecognized pipe name still released
- **IF** a CLI pipe message arrives with a name the plugin does not handle
- **THEN** the plugin SHALL still unblock that pipe input (no-op handling
  never strands the client)

#### Scenario: non-CLI pipes unaffected
- **WHEN** a pipe message arrives from a non-CLI source (plugin-to-plugin)
- **THEN** the plugin SHALL NOT issue a CLI-pipe unblock for it

---

## Acceptance criterion quality checklist

| AC ID | Testable | Solution-free | Unambiguous | Consistent | Complete |
|---|---|---|---|---|---|
| pane-pipe-api.cli-pipe-client-release | [x] | [x] | [x] | [x] | [x] |
