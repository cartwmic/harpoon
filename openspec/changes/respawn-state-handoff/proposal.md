# Proposal — respawn-state-handoff

## Why

Cross-tab `Ctrl+y` invokes intermittently show an empty/partial jump-target
list (targets reappear on a later invoke): the respawned instance's menu
renders ~55ms after load while the bookmark disk load needs three async hops
(F1). Triage also exposed a latent prune-then-save path that could turn the
display race into real disk loss (F2), and a restore fallback keyed on
volatile pane titles that misses live panes after session restarts (F3).
Frozen intent: `intent.md` (commit `7409760`), mechanism source-verified and
spike-probed (`/tmp/spike-handoff/`, 2026-07-13).

## What Changes

- **F1 — targeted bootstrap hand-off:** the Respawn branch captures the
  successor's pane id from `open_plugin_pane_floating`'s return value
  (currently discarded), sends a `bootstrap_store` pipe via `MessageToPlugin`
  `destination_plugin_id` routing (serialized `BookmarkStore` + session
  name), then `close_self()`. Successor adopts the payload instantly; the
  existing disk load remains as cold-boot fallback and reconciliation input.
  Adoption handler is deny-safe (pipe arrives pre-grant — probe-proven) and
  pure-state.
- **New permission:** `MessageAndLaunchOtherPlugins` added to
  `request_permission`, script permission seeds, and README (runtime regrant
  documented). Denied/absent grant degrades to today's disk-load path.
- **Duplicate-delivery tolerance:** a stale invocation pipe re-delivered to
  the successor (~380ms after respawn — probe-proven) MUST NOT hide the
  just-shown menu.
- **F2 — prune-guard:** shrinking (bookmark-removing) saves are forbidden
  until the instance has observed BOTH a resolved disk load AND a full pane
  manifest; additive saves stay allowed.
- **F3 — restore-fallback hardening:** reduce dependence on volatile
  `(tab_name, pane_title)` matching without regressing the existing
  `reorder` restore semantics (restore-freeze, placeholder slots,
  best-effort non-unique identity).
- All decision logic (adoption precedence, duplicate tolerance, save guard,
  restore matching) lands in `harpoon-core` with native tests (Constitution
  I); shim wires host calls only.
- Regression script extended: instant-targets-on-respawn, duplicate-pipe
  tolerance, prune-guard early-save window, permission-denied degrade.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pane-pipe-api`: amend "Toggle Pipe Invocation" — respawn hands off state
  to the successor (bootstrap pipe); add duplicate-delivery tolerance and
  permission-degrade scenarios.
- `reorder`: persistence deltas — destructive-save prune-guard; restore
  fallback hardened against volatile titles.

## Impact

- **Affected files:** `harpoon-core/src/pipe_api.rs`,
  `harpoon-core/src/restore.rs`, `harpoon-core/src/lib.rs`,
  `harpoon-plugin/src/main.rs`, `harpoon-plugin/src/persistence.rs`,
  `scripts/toggle-pipe-regression.sh`, `README.md`,
  `openspec/changes/respawn-state-handoff/specs/{pane-pipe-api,reorder}/`.
- **Dependencies:** zellij-tile 0.44.3 (unchanged); no new crates expected.
- **Runtime (operational, outside gate):** deploy wasm; answer the new
  `MessageAndLaunchOtherPlugins` prompt in a visible pane; verify a
  cross-tab invoke shows the full list instantly.
- **Failure envelope:** spawn returns `None`/non-plugin id, bootstrap lost,
  permission denied — all degrade to the existing disk-load path; never
  worse than today.

## Open Questions

- **Q1 — bootstrap payload format?** A: reuse the persistence v2 JSON
  envelope (`{version:2, bookmarks:[...]}`) + session name in pipe args. B:
  new bespoke payload struct. **Resolved: A** — one serializer, already
  round-trip-tested, version field future-proofs the pipe.
- **Q2 — F3 mechanism?** A: persist/carry stable pane ids in the hand-off so
  restart-survival stops depending on titles; title fallback demoted to
  last resort. B: fuzzy title matching. **Resolved: A** — B adds
  wrong-pane-match risk, violating restore best-effort semantics; exact
  mechanism detailed at design/spec level within the frozen constraint.
- **Q3 — duplicate-toggle window?** A: successor ignores toggle pipes until
  first render completes post-bootstrap-or-load. B: time-based debounce.
  **Resolved: A** — deterministic state condition, natively testable in
  core; timers are the flakier path (established invariant: suppressed panes
  get no events, timers already overloaded).
