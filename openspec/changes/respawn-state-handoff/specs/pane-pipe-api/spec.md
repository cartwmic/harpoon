# Capability: pane-pipe-api (delta — respawn-state-handoff)

## ADDED Requirements

### Requirement: Respawn State Hand-Off

THE outgoing instance SHALL hand its in-memory bookmark state directly to
the successor WHEN the `toggle` pipe's respawn branch opens a fresh
instance on the invoking tab, instead of leaving the successor to race the
disk load: it SHALL capture the successor's pane id from the spawn host call's
return value, send a `bootstrap_store` pipe message routed by destination
plugin id (never by url+configuration matching) whose payload carries the
serialized bookmark store (persistence v2 envelope) and the session name,
and only then close itself. The successor SHALL adopt the payload as its
in-memory store immediately on receipt. Adoption SHALL be deny-safe pure
state mutation only — no response-decoding host calls — because the
bootstrap message can arrive before the successor's permission grant.

Adoption-vs-disk precedence SHALL be pure decision logic in `harpoon-core`
(Constitution I): an adopted bootstrap payload wins while the successor's
disk load is unresolved; a disk load result arriving after adoption SHALL
reconcile and SHALL NOT clobber newer in-memory mutations. The disk load
remains the cold-boot fallback whenever no bootstrap payload arrives.

#### Scenario: cross-tab respawn presents persisted targets immediately
- **GIVEN** the plugin is hidden and parked on tab A with a non-empty
  bookmark store
- **WHEN** a `toggle` pipe message arrives while the client's active tab is
  B (B ≠ A) and the respawn branch runs
- **THEN** the successor's menu SHALL present the full persisted
  jump-target list on its first render (no empty-list flash, no
  invoke-again-to-recover)

#### Scenario: bootstrap arriving pre-grant is adopted safely
- **GIVEN** the successor has loaded but its permission grant has not yet
  been confirmed (`PermissionRequestResult` not received)
- **WHEN** the `bootstrap_store` pipe message arrives
- **THEN** the successor SHALL adopt the payload via pure state mutation
  only, making no response-decoding host call in the adoption path
- **AND** the plugin SHALL NOT panic

#### Scenario: late disk load does not clobber newer mutations
- **GIVEN** the successor adopted a bootstrap payload and the user then
  mutated the store (e.g. added a bookmark)
- **WHEN** the successor's own disk load result arrives afterwards
- **THEN** the in-memory store SHALL retain the user's newer mutation (the
  stale disk result reconciles; it never replaces newer in-memory state)

#### Scenario: spawn id unavailable degrades to disk load
- **IF** the spawn host call returns no pane id (or a non-plugin id)
- **THEN** the outgoing instance SHALL skip the bootstrap send and close
  itself as today
- **AND** the successor SHALL populate its store via the existing disk-load
  path (behavior never worse than the status quo)

#### Scenario: bootstrap send denied degrades to disk load
- **IF** the permission required for the bootstrap send is denied or not
  yet granted at send time
- **THEN** the outgoing instance SHALL NOT invoke the gated host call
  (no panic; response-decoding host calls are grant-gated), SHALL skip the
  hand-off, and the successor SHALL fall back to the disk-load path

### Requirement: Duplicate Toggle Delivery Tolerance

THE successor SHALL NOT hide its just-shown menu in response to a stale
toggle: IF a queued or stale `toggle` pipe message is re-delivered to a
freshly respawned successor before the user has interacted with the
just-shown menu, THEN the successor SHALL ignore it while still releasing
the blocked CLI client (probe evidence 2026-07-13: the in-flight CLI
invocation pipe that triggered the respawn is re-delivered to the successor
~380ms after load, carrying the SAME pipe id). The ignore-vs-honor decision
SHALL be pure decision logic in `harpoon-core`, keyed on deterministic pipe
identity — the hand-off payload carries the pipe id the sender already
handled and the successor ignores exactly one toggle from that source —
never on wall-clock debounce (and never on a shown/readiness proxy: the
re-delivery arrives AFTER the menu is shown, so timing/readiness conditions
cannot distinguish it from a genuine re-invoke).

#### Scenario: re-delivered invocation pipe does not hide the menu
- **GIVEN** the respawn branch just spawned a successor and the successor
  has shown its menu
- **WHEN** the original in-flight `toggle` pipe message is re-delivered to
  the successor immediately after load (same CLI pipe id the outgoing
  instance already handled)
- **THEN** the menu SHALL remain shown (the stale toggle is ignored)
- **AND** the blocked CLI pipe client SHALL still be released

#### Scenario: genuine user re-invoke still hides
- **GIVEN** the successor's menu is shown
- **WHEN** the user presses the keybind again (a new `toggle` pipe message —
  keybind-sourced or a NEW CLI pipe id, never the sender-handled id)
- **THEN** the visible-and-focused hide branch SHALL run as specified by
  Toggle Pipe Invocation

## MODIFIED Requirements

### Requirement: Host Call Permission Completeness

THE plugin SHALL request, at load, every `PermissionType` required by a host
call it invokes — including `ReadCliPipes` (required for
`unblock_cli_pipe_input` / `cli_pipe_output`), `OpenTerminalsOrPlugins`
(required for the respawn branch's plugin-pane spawn), and
`MessageAndLaunchOtherPlugins` (required for the `bootstrap_store` hand-off
send) — so that no host call the plugin depends on is silently
permission-denied at runtime.

#### Scenario: ReadCliPipes requested at load
- **WHEN** the plugin's `load()` runs
- **THEN** the permission request SHALL include `ReadCliPipes` alongside the
  existing `RunCommands`, `ReadApplicationState`, and
  `ChangeApplicationState` permissions

#### Scenario: MessageAndLaunchOtherPlugins requested at load
- **WHEN** the plugin's `load()` runs
- **THEN** the permission request SHALL include
  `MessageAndLaunchOtherPlugins` and `OpenTerminalsOrPlugins`

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
- **THEN** the plugin SHALL NOT treat any gated host-call behavior
  (pipe-release, pipe-output, plugin-pane spawn, bootstrap send) as
  available (grant is verified, never assumed — Constitution IV)

---

## Acceptance criterion quality checklist

| AC ID | Testable | Solution-free | Unambiguous | Consistent | Complete |
|---|---|---|---|---|---|
| pane-pipe-api.respawn-state-hand-off | [x] | [x] (names host-call surface only where identity/permission semantics force it) | [x] | [x] | [x] (nominal + pre-grant + late-load + both failure degrades) |
| pane-pipe-api.duplicate-toggle-delivery-tolerance | [x] | [x] | [x] | [x] (genuine-re-invoke scenario reconciles with hide branch) | [x] |
| pane-pipe-api.host-call-permission-completeness | [x] | [x] | [x] | [x] | [x] |
