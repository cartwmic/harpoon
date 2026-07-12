# Intent — pipe-toggle-invocation

**Status:** FROZEN (explore concluded 2026-07-11)
**Recommended Scale:** M, `full_rigor: false`

## Intent

Invoking harpoon via the `Ctrl+y` keybind (`LaunchOrFocusPlugin`)
mis-navigates the user roughly half the time in long-lived sessions:
the view jumps to an unrelated tab (observed: "Amazon") while the menu
opens elsewhere. Root cause is a double defect in zellij's
`focus_plugin_pane` (`zellij-server/src/screen.rs:3918`, v0.44.3 and
current upstream `main`):

1. **id/position confusion:** `go_to_tab(tab_index + 1)` passes the
   stable tab **id** (the `tabs: BTreeMap<usize, Tab>` key) to an API
   expecting **position + 1**. Ids never renumber; positions compact on
   tab close — after tab churn the view lands on whatever tab sits at
   position == the plugin-home-tab's id. This violates the id/position
   convention documented in that file's own header ("index: synonym for
   position (used in public/plugin APIs)").
2. **suppressed-pane move no-op:** the `move_to_focused_tab` branch
   calls `extract_pane(pane_id, dont_swap_if_suppressed=true)`, which
   returns `None` for suppressed panes — and `hide_self()` (harpoon's
   Esc-close path) parks the pane in `suppressed_panes`. Cross-tab
   invokes therefore never move the pane; they silently fall through
   into defect 1. This is a regression introduced by upstream's own fix
   (PR #3841, Dec 2024) for the same symptom reported against the
   session-manager (issue #3834, still broken per later user reports).

Replace the invocation mechanism so the broken upstream path is never
executed: change the keybind from `LaunchOrFocusPlugin` to
`MessagePlugin` (a `toggle` pipe), and have harpoon own its show/hide
lifecycle using host calls verified correct in the zellij 0.44.3
source — `show_self(true)` (routes through
`screen.focus_pane_with_id`: locates the pane's tab including
suppressed panes, navigates by `tab.position`, un-suppresses and
re-adds floating) and `break_panes_to_tab_with_index` (position-correct
body) for cross-tab relocation.

User-observable outcome: invoking harpoon SHALL present the menu as a
floating pane on the tab the user invoked it from, with the view
remaining on that tab — regardless of which tab the hidden instance was
parked on, and regardless of prior tab closes (id/position drift).

## Why D2 (decision record)

Explored alternatives, all rejected:

- **A — revert `hide_self()` to `close_self()`** (the d6a2039-era
  workaround): only partial. It removes the suppressed-pane supplier
  but ntfy pipe cold-spawns (live since 2026-07-05) still park
  unfocused floating harpoon panes in the active tab, which
  `find_plugin` matches — the bug persists at the Jul 5–9 frequency.
  Also reintroduces the 47–92ms cold wasm load per invocation and
  re-exposes the cold-start pipe race (pipe arrives ~55ms before the
  first TabUpdate/PaneUpdate).
- **B — patch a zellij fork:** complete fix (two lines) but perpetual
  rebase burden on a large fast-moving repo, and swaps a mise-managed
  binary for a self-built one. Disproportionate to the defect.
- **C — upstream issue/PR:** correct long-term and pursued in parallel
  (non-goal here), but upstream history is discouraging — issue #3834
  got a maintainer fix (PR #3841) that introduced defect 2, a user
  reported it still broken, and it has sat dead since Dec 2024. Zellij
  release cadence means months of exposure even if merged promptly.
- **D1 — detection-based self-heal:** let the broken path run, detect
  the anomaly (`Event::Visible` + TabUpdate mismatch), correct
  after-the-fact. Racy, flickery, must model both defects' anomaly
  windows. Obsoleted by D2, which prevents instead of heals.

**D2 — pipe-toggle bypass — chosen because it is the only option that
is simultaneously:** complete (both defects unreachable — the broken
function is simply never called), supplier-agnostic (hide_self
suppressed panes AND ntfy cold-spawn floating panes handled
identically), warm (no per-invocation wasm load), self-contained (no
zellij fork, no upstream wait), and cold-start-safe (`show_self` is a
host-side call needing no cached event state, unlike the jump path).
Side benefit: keybind and ntfy both address the same warm instance via
pipes, collapsing the `--plugin-configuration`-must-match foot-gun.

## Constraints

- Toggle handler branches (design pins mechanism; these are the
  behavioral obligations):
  - visible → hide (Esc-close semantics unchanged: `hide_self()`,
    mode-state-machine "Close consolidation" untouched);
  - hidden, parked on the active tab → show in place;
  - hidden, parked on another tab → relocate to the active tab, then
    show there (sequencing note: `break_panes_to_tab_with_index` cannot
    extract suppressed panes — un-suppress via `show_self(true)` MUST
    precede relocation);
  - not loaded (cold spawn from the pipe) → show; MUST NOT depend on
    cached TabUpdate/PaneUpdate state (Constitution IV — the pipe
    arrives before the first events on cold spawn).
- Requires subscribing to `Event::Visible` (zellij-utils
  `data.rs:962`) for visibility-state tracking; harpoon currently does
  not subscribe to it. Visibility state MUST be event-derived, not
  assumed from command history.
- Open risks from explore MUST be resolved in-change with recorded
  evidence before the change is considered done:
  - R1: cross-tab show-then-relocate flicker (two screen instructions,
    each renders) — measure/observe; acceptable if at most a brief
    single-frame artifact, escalate to the user if worse;
  - R2: keybind-source (`MessagePlugin`) pipe delivery and permission
    semantics — CLI-source pipes needed `ReadCliPipes`
    (2026-07-10-request-read-cli-pipes-permission); verify keybind
    pipes reach the plugin and whether any permission is required;
  - R3: whether suppressed panes appear in `PaneManifest` for own-tab
    detection — if not, fall back to unconditional
    show-then-relocate-if-needed.
- Spec delta required: `pane-pipe-api` gains a toggle-pipe requirement
  (invocation via named pipe, the four handler branches, cross-tab
  relocation scenario). `mode-state-machine` deltas only if the close
  path is touched (it should not be).
- Regression evidence: committed scenario(s) in the established
  tmux-hosted harness style (precedent:
  `scripts/fullscreen-regression.sh`) covering at minimum: cross-tab
  invoke lands menu+view on the invoking tab after tab churn (a closed
  tab forcing id/position drift), and same-tab re-invoke after
  Esc-close. Native `harpoon-core` tests cover any extracted pure
  decision logic (toggle branch selection).
- Build target wasm32-wasip1 only (Constitution III).
- Runtime activation is operational and OUTSIDE gate assertions but
  MUST be documented (tasks or README): deploy wasm; update the zellij
  keybind in the chezmoi-managed `~/.config/zellij/config.kdl`
  (`LaunchOrFocusPlugin` block → `MessagePlugin` with the `toggle`
  pipe name and matching plugin URL/config); reload zellij config or
  restart the server; verify a warm toggle round-trip.

## Invariants honored

- Constitution I (core/shim split is sacred): toggle branch selection
  (visible/hidden-same-tab/hidden-cross-tab/cold) is pure decision logic
  and lands in `harpoon-core` with native tests; only the host calls
  (`hide_self`, `show_self`, `break_panes_to_tab_with_index`,
  subscriptions) live in the plugin shim.
- Constitution II (specs are the source of behavior): the toggle pipe
  lands as a `pane-pipe-api` requirement alongside the code.
- Constitution III (wasm32-wasip1 is the only build target).
- Constitution IV (never act on unverified host state): cold-spawn
  branch works without cached event state; visibility state comes from
  `Event::Visible`, not assumption.
- Constitution V (canonical effect ordering): jump/fullscreen effect
  ordering untouched; the new show-then-relocate ordering is specified,
  not incidental.
- Existing `pane-pipe-api` requirements (Targeted Pipe Delivery, Cli
  Pipe Client Release, Host Call Permission Completeness) remain as
  specified; the toggle pipe must satisfy Permission Completeness for
  any newly exercised host calls.

## Non-goals

- Filing the upstream zellij issue/PR (option C) — pursued in parallel,
  not gated by or gating this change.
- Any zellij fork or local zellij patch (option B).
- Reverting to `close_self()` (option A) — explicitly rejected; if D2
  hits a showstopper (R1/R2), that is a new explore, not a silent
  fallback.
- Changes to the ntfy jump path (`jump_pane` pipe, zellij-jump script,
  termux side) beyond the shared-instance benefit falling out of the
  keybind change.
- Esc-close semantics or mode-state-machine behavior changes.
- Multi-client or multi-session support for the toggle pipe.
- Removing the parked-pane behavior of ntfy cold-spawns (harmless under
  D2: the toggle handler relocates or shows correctly wherever the
  instance is parked).
