# Proposal — pipe-toggle-invocation

<!-- authored: in-session -->

## Why

Invoking harpoon via the `Ctrl+y` `LaunchOrFocusPlugin` keybind mis-navigates
roughly half the time in long-lived sessions: the view jumps to an unrelated
tab while the menu opens elsewhere. Root cause is a double defect in zellij's
`focus_plugin_pane` (`zellij-server/src/screen.rs:3918`, v0.44.3 and current
upstream `main`): (1) `go_to_tab(tab_index + 1)` passes a stable tab **id**
to a **position**-based API, so after tab churn the view lands on whatever tab
sits at position == the plugin-home-tab's id; (2) the `move_to_focused_tab`
branch calls `extract_pane(id, dont_swap_if_suppressed=true)` which returns
`None` for suppressed panes (harpoon's `hide_self()` Esc-close parks the pane
in `suppressed_panes`), so cross-tab invokes silently fall through into
defect 1 — a regression introduced by upstream PR #3841 (Dec 2024), which was
itself the fix for the same symptom (issue #3834, still broken per later user
reports). Upstream has been dead on this for ~19 months; waiting is not a fix.

The frozen intent (D2 decision record) selects the only complete,
supplier-agnostic, warm, self-contained option: stop calling the broken
function entirely.

## What Changes

- Replace the invocation mechanism: the zellij keybind changes from
  `LaunchOrFocusPlugin` to `MessagePlugin` (a named `toggle` pipe), and
  harpoon owns its show/hide lifecycle. The broken upstream
  `focus_plugin_pane` path is never executed.
- New `toggle` pipe handler with four behavioral branches (frozen in
  intent.md):
  - visible → hide (`hide_self()`; Esc-close semantics and
    mode-state-machine "Close consolidation" untouched);
  - hidden, parked on the active tab → show in place (`show_self(true)`);
  - hidden, parked on another tab → un-suppress via `show_self(true)` FIRST,
    then relocate to the active tab via `break_panes_to_tab_with_index`
    (which cannot extract suppressed panes — sequencing is mandatory);
  - not loaded (cold spawn from the pipe) → `show_self(true)`; MUST NOT
    depend on cached TabUpdate/PaneUpdate state (Constitution IV — the pipe
    arrives before the first events on cold spawn).
- Subscribe to `Event::Visible` and derive visibility state from it (never
  from command history).
- Toggle branch selection lands as pure decision logic in `harpoon-core`
  with native tests; host calls stay in the plugin shim (Constitution I).
- `pane-pipe-api` spec delta: toggle-pipe requirement (invocation via named
  pipe, the four branches, cross-tab relocation scenario).
- Committed tmux-hosted regression scenario(s) (precedent:
  `scripts/fullscreen-regression.sh`) covering at minimum: cross-tab invoke
  lands menu+view on the invoking tab after tab churn (a closed tab forcing
  id/position drift), and same-tab re-invoke after Esc-close.
- In-change resolution, with recorded evidence, of the three frozen risks:
  R1 cross-tab show-then-relocate flicker (observe; escalate if worse than a
  brief single-frame artifact), R2 keybind-source pipe delivery/permission
  semantics, R3 suppressed-pane visibility in `PaneManifest` (fallback:
  unconditional show-then-relocate-if-needed).
- Document runtime activation (operational, outside gate assertions): deploy
  wasm; update the chezmoi-managed `~/.config/zellij/config.kdl` keybind
  (`LaunchOrFocusPlugin` block → `MessagePlugin` with the `toggle` pipe name
  and matching plugin URL/config); reload zellij config; verify a warm
  toggle round-trip.

No breaking changes to existing pipes (`jump_pane`, `slot_for_pane`) or to
bookmark/mode behavior.

## Clarifications (folded, plain Scale M)

- **Cross-tab UX target:** menu + view end on the *invoking* tab (preserves
  today's intended `move_to_focused_tab true` semantics) — pinned by the
  frozen intent's user-observable outcome; "go to the menu's home tab" was
  considered and rejected during explore.
- **Esc path:** stays `hide_self()`. D2 replaces only the *open* side; the
  hidden pane parking on the last-used tab is handled by the cross-tab
  branch.
- **Keybind config residency:** the keybind lives in the chezmoi-managed
  zellij config, outside this repo — documented as runtime activation, not
  gated here.
- **R1 acceptability threshold:** at most a brief single-frame artifact on
  cross-tab relocation; anything worse escalates to the user (frozen in
  intent constraints).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pane-pipe-api`: new requirement — Toggle Pipe Invocation (named `toggle`
  pipe drives show/hide; four behavioral branches; cross-tab relocation with
  mandatory un-suppress-before-relocate sequencing; cold-spawn branch free of
  cached-event-state dependence). Existing requirements (Targeted Pipe
  Delivery, Cli Pipe Client Release, Host Call Permission Completeness)
  unchanged; the toggle pipe must satisfy Permission Completeness for any
  newly exercised host calls.

## Impact

- **Affected files:**
  - `harpoon-core/src/` — new pure toggle-branch decision logic + native
    tests (Constitution I core/shim split)
  - `harpoon-plugin/src/main.rs` — `toggle` pipe handling, `Event::Visible`
    subscription + visibility state, host-call wiring (`show_self`,
    `break_panes_to_tab_with_index`)
  - `openspec/specs/pane-pipe-api/spec.md` — delta requirement (via change
    `specs/pane-pipe-api/spec.md`)
  - `scripts/` — new committed tmux-hosted regression scenario(s)
  - README — runtime activation documentation (keybind swap)
- **Build:** wasm32-wasip1 only (Constitution III). Native `harpoon-core`
  tests cover branch selection; host behavior (pipe delivery, relocation,
  flicker) is covered by the scenario scripts.
- **Out of repo (documented, not gated):** chezmoi `config.kdl` keybind
  change.
- **Non-goals (frozen):** upstream issue/PR filing (parallel track), zellij
  fork, `close_self()` revert, ntfy jump-path changes, Esc semantics,
  multi-client/multi-session toggle support.
