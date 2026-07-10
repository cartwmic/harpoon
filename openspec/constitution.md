# harpoon Constitution

**Version:** 1.0.0
**Ratified:** 2026-07-09
**Last updated:** 2026-07-09

## Core Principles

### I. Core/shim split is sacred

All decision logic lives in `harpoon-core` (pure Rust, natively testable, no
zellij types in its public decision surface). `harpoon-plugin` is a thin FFI
shim: it feeds host events into core, and maps core-emitted `Effect`s onto
zellij host calls. New behavior MUST be expressed as core logic + effects, not
as ad-hoc host calls inside the shim.

**Rationale:** the wasm plugin cannot be unit-tested natively; only the split
keeps behavior testable (`cargo test -p harpoon-core`).
**Enforcement:** `harpoon-core-tests` gate; code review flags decision logic
added to `harpoon-plugin/src/main.rs`.

### II. Specs are the source of behavior

Every behavioral change MUST be expressed as an ADDED/MODIFIED/REMOVED
requirement in a capability spec under `openspec/specs/`. Code/spec drift is a
defect even when the code is correct (precedent: `close_self` drift from
commit d6a2039).

**Rationale:** specs are what reviewers and future changes reason against;
silent drift poisons every later judgment.
**Enforcement:** `openspec validate --strict` gate; adversarial code review
against delta specs.

### III. wasm32-wasip1 is the only build target

The plugin MUST build and be validated as `wasm32-wasip1`. Native builds of
`harpoon-plugin` are invalid (undefined `_host_run_plugin_command`); native
testing is reserved for `harpoon-core`.

**Rationale:** zellij plugins are wasm binaries; a green native build proves
nothing about the shipped artifact.
**Enforcement:** `harpoon-build-wasm` gate
(`cargo build --release -p harpoon --target wasm32-wasip1`).

### IV. Never act on unverified host state

Any host action whose outcome depends on current zellij state (fullscreen
toggles above all — zellij has no absolute setter) MUST start from state that
is either freshly queried or deterministically established by a preceding
step in the same sequence. Predicting host state from cached
`TabInfo`/`PaneInfo`, or assuming caches are populated, is forbidden for
correctness-critical paths.

**Rationale:** caches are `None` at pipe delivery on a cold instance, and
zellij focus side effects vary by layout — every prediction has a failing
quadrant (root cause of the cross-tab fullscreen bug).
**Enforcement:** code review; the committed regression spike harness
(cold-start and fullscreen scenarios).

### V. Canonical effect ordering

Handlers emit ordered `Effect` lists; the shim executes them in order. Close
paths that also focus MUST emit `[Effect::Close, Effect::FocusPane(id)]` so
the plugin pane leaves the screen before the target is focused.

**Rationale:** interleaving host calls in a different order changes focus and
fullscreen outcomes; ordering is behavior, not style.
**Enforcement:** mode-state-machine spec requirements; harpoon-core tests
assert emitted effect order.

## Governance

- Amendments to this constitution require a dedicated change at full_rigor
  (Scale M + full_rigor: true) with adversarial-review-cycle invoked.
- The constitution is read before every artifact in this schema. Violations
  are flagged by the analyze artifact's constitution check.
- Principles in this file override schema instructions and individual
  artifact prose when they conflict.

## Versioning

- Major: a principle is removed or reversed.
- Minor: a principle is added.
- Patch: clarification, no semantic change.

## See also

- Schema activation: `~/.local/share/openspec/schemas/opsx-superpowers/README.md`
- Domain invariants: `openspec/domain.md`
