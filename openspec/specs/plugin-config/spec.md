# plugin-config Specification

## Purpose
TBD - created by archiving change add-filter-and-jump-modes. Update Purpose after archive.
## Requirements
### Requirement: Read config on load

The plugin's `ZellijPlugin::load` implementation SHALL read configuration values from the `BTreeMap<String, String>` argument and store them in a `Config` struct on `State`. Reading SHALL be tolerant of missing keys (use defaults) and unknown values (log to stderr and use defaults).

#### Scenario: Config keys are read from load argument
- **GIVEN** the plugin is loaded with config map `{"default_mode": "filter", "matcher": "substring", "show_slots": "false"}`
- **WHEN** `load` returns
- **THEN** `State.config.default_mode == Filter`
- **AND** `State.config.matcher == Substring`
- **AND** `State.config.show_slots == false`

#### Scenario: Empty config uses all defaults
- **GIVEN** the plugin is loaded with an empty config map
- **WHEN** `load` returns
- **THEN** `State.config.default_mode == Command`
- **AND** `State.config.matcher == Fuzzy`
- **AND** `State.config.show_slots == true`

### Requirement: Config defaults

When a config key is missing OR has an unrecognized value, the plugin SHALL use these defaults: `default_mode = command`, `matcher = fuzzy`, `show_slots = true`.

#### Scenario: Missing key uses default
- **GIVEN** the plugin is loaded with config `{"default_mode": "jump"}`
- **WHEN** `load` returns
- **THEN** `State.config.matcher == Fuzzy`
- **AND** `State.config.show_slots == true`

#### Scenario: Unknown value falls back to default
- **GIVEN** the plugin is loaded with config `{"matcher": "regex"}`
- **WHEN** `load` returns
- **THEN** `State.config.matcher == Fuzzy`
- **AND** a warning is written to stderr referencing the rejected value

### Requirement: default_mode accepts only three values, case-insensitively

`default_mode` SHALL accept only the strings `command`, `filter`, `jump`, compared with ASCII case-insensitive equality. Any other value SHALL fall back to `command`.

#### Scenario: Valid mode strings parse
- **WHEN** `default_mode` is `"command"`, `"filter"`, or `"jump"`
- **THEN** the corresponding `Mode` value is stored

#### Scenario: Mixed case accepted
- **GIVEN** config has `"default_mode": "Filter"`
- **WHEN** `load` returns
- **THEN** `State.config.default_mode == Filter`

#### Scenario: Garbage value falls back
- **GIVEN** config has `"default_mode": "wibble"`
- **WHEN** `load` returns
- **THEN** `State.config.default_mode == Command`

### Requirement: matcher accepts only two values, case-insensitively

`matcher` SHALL accept only the strings `fuzzy` (default) or `substring`, compared with ASCII case-insensitive equality. Any other value SHALL fall back to `fuzzy`.

#### Scenario: substring matcher selectable
- **GIVEN** config has `"matcher": "substring"`
- **WHEN** `load` returns
- **THEN** `State.config.matcher == Substring`

#### Scenario: Mixed case accepted
- **GIVEN** config has `"matcher": "Fuzzy"`
- **WHEN** `load` returns
- **THEN** `State.config.matcher == Fuzzy`

### Requirement: show_slots accepts boolean strings, case-insensitively

`show_slots` SHALL accept the strings `"true"` and `"false"`, compared with ASCII case-insensitive equality. Any other value SHALL fall back to `true`. Parsing matches the case behavior of `default_mode` and `matcher` for consistency.

#### Scenario: false disables slot prefixes
- **GIVEN** config has `"show_slots": "false"`
- **WHEN** `load` returns
- **THEN** `State.config.show_slots == false`
- **AND** rendered rows have no slot prefix

#### Scenario: Mixed case accepted
- **GIVEN** config has `"show_slots": "FALSE"`
- **WHEN** `load` returns
- **THEN** `State.config.show_slots == false`

#### Scenario: Garbage value falls back to true
- **GIVEN** config has `"show_slots": "yes"`
- **WHEN** `load` returns
- **THEN** `State.config.show_slots == true`

### Requirement: Config is read once at load

The plugin SHALL read config only in its `load` callback. The plugin instance survives `hide_self()` (verified by Phase 0 task 0.2), so opening and closing harpoon within a session does NOT trigger a re-read. Re-reading only occurs when zellij re-instantiates the plugin, which happens on zellij host restart, plugin reload via the host's plugin-manager command, or plugin update.

#### Scenario: Config not re-read mid-session via show/hide
- **GIVEN** the plugin has been loaded with `default_mode = command`
- **AND** the user opens and closes harpoon multiple times
- **WHEN** the user edits the kdl config to `default_mode = filter` between opens
- **THEN** `State.config.default_mode` remains `Command` for the lifetime of the existing plugin instance

#### Scenario: New plugin instance via host re-instantiation picks up new config
- **GIVEN** the user updates the kdl with `default_mode = filter`
- **AND** the host re-instantiates the plugin (zellij restart, plugin-manager reload, or new session)
- **WHEN** the new instance's `load()` runs
- **THEN** it reads `default_mode = filter`

#### Scenario: Phase 0 finding overrides this requirement if instance does not survive
- **GIVEN** Phase 0 task 0.2 reveals the plugin instance is destroyed on `hide_self()` (the design's assumption fails)
- **THEN** the close-helper reset and `default_mode` semantics in `mode-state-machine/spec.md` are revisited as part of the Phase 0 contingency
- **AND** `load()` would in fact run on every open, so config re-reads naturally

