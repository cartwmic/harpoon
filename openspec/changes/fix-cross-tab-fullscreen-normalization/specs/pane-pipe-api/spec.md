# Capability: pane-pipe-api

## ADDED Requirements

### Requirement: Ground Truth Fullscreen Normalization

WHEN a jump requires a fullscreen toggle decision, THE plugin SHALL derive the
tab's fullscreen state from ground truth current at decision time (a fresh
post-focus query of the host, or a state deterministically established by an
immediately preceding step of the same jump sequence) — never from
event-cached `TabInfo`/`PaneInfo` snapshots or from predictions of the host's
focus side effects.

#### Scenario: cold-start pipe decides from ground truth
- **WHEN** a `jump_pane` pipe is delivered to a freshly loaded plugin instance
  whose `TabUpdate`/`PaneUpdate` caches are still empty
- **THEN** the plugin SHALL resolve the target tab's actual fullscreen state
  before issuing any fullscreen toggle
- **AND** the jump outcome SHALL be identical to the warm-cache outcome

#### Scenario: no toggle from unknown state
- **IF** the tab's current fullscreen state cannot be established
- **THEN** the plugin SHALL NOT issue a fullscreen toggle for that jump

### Requirement: Targeted Pipe Delivery

THE plugin's documentation and any shipped invocation examples SHALL instruct
callers to target `jump_pane` pipes at the plugin explicitly (`--plugin`,
optionally with `--plugin-configuration`), and SHALL NOT recommend broadcast
pipes (no `--plugin` target) for `jump_pane`.

#### Scenario: README example is targeted
- **WHEN** the README documents invoking `jump_pane` over the CLI pipe
- **THEN** the example SHALL pass an explicit `--plugin` target

#### Scenario: broadcast hazard documented
- **IF** more than one harpoon instance is loaded (distinct configurations)
- **THEN** the documentation SHALL state that a broadcast `jump_pane` reaches
  all instances and risks fullscreen double-toggle cancellation

## MODIFIED Requirements

### Requirement: Jump To Pane By Id

THE plugin SHALL answer a CLI pipe message named `jump_pane` carrying a
resolvable pane id by focusing that terminal pane and its tab and leaving the
target pane fullscreen, with the outcome correct in every combination of
plain/stacked fullscreen layout, same-tab/cross-tab origin, and warm/cold
plugin state cache.

#### Scenario: jump focuses the target pane
- **WHEN** a `jump_pane` pipe carries a resolvable terminal pane id present in
  the live session
- **THEN** the plugin SHALL focus that terminal pane and its containing tab
- **AND** the target pane SHALL end fullscreen in its tab

#### Scenario: cross-tab jump into a fullscreen tab
- **WHILE** the target pane's tab is inactive and fullscreen on a different
  pane
- **WHEN** a `jump_pane` pipe targets a hidden pane of that tab
- **THEN** the plugin SHALL end with the target pane focused and fullscreen

#### Scenario: cold-start pipe into a fullscreen tab
- **WHEN** a `jump_pane` pipe is delivered to a freshly loaded instance (empty
  state caches) targeting a pane in a fullscreen tab
- **THEN** the plugin SHALL end with the target pane focused and fullscreen

#### Scenario: unresolvable target
- **IF** a `jump_pane` pipe carries a payload that does not resolve to a pane
  id
- **THEN** the plugin SHALL NOT change focus or fullscreen state

---

## Acceptance criterion quality checklist

| AC ID | Testable | Solution-free | Unambiguous | Consistent | Complete |
|---|---|---|---|---|---|
| pane-pipe-api.ground-truth-fullscreen-normalization | [x] | [x] | [x] | [x] | [x] |
| pane-pipe-api.targeted-pipe-delivery | [x] | [x] | [x] | [x] | [x] |
| pane-pipe-api.jump-to-pane-by-id | [x] | [x] | [x] | [x] | [x] |
