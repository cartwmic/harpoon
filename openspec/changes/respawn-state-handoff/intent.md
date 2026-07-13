# Intent — respawn-state-handoff

**Status:** FROZEN (explore concluded 2026-07-13)
**Recommended Scale:** M, `full_rigor: false`

## Intent

Since the pipe-toggle deployment (2026-07-12, archive
`2026-07-12-pipe-toggle-invocation`), cross-tab invokes intermittently
present an **empty or partial jump-target list**; the targets reappear
on a later invoke. Root cause is a cold-start race inherent to the
respawn mechanism, plus two adjacent persistence defects found during
triage:

1. **F1 — bootstrap race (the symptom):** every cross-tab invoke
   respawns a fresh plugin instance whose bookmark store starts empty.
   Populating it takes three async hops — `Event::SessionUpdate`
   (learn session name, `main.rs:170-173`) → `run_command("cat
   ~/.local/share/zellij-harpoon/<session>.json")`
   (`persistence.rs:66-73`) → `Event::RunCommandResult`
   (`main.rs:177-188`) — while the toggle pipe shows the menu ~55ms
   after load. Render beats the load chain → empty list. Same-tab
   warm invokes (`show_self`, in-memory store) never lose targets;
   only respawns do. Disk was verified current at triage time — the
   loss is display-only, not data loss.
2. **F2 — prune-then-save hole:** `update_panes` reconciliation drops
   bookmarks for panes absent from the pane manifest, then
   `save_if_changed` persists (`main.rs:~436-519`). The empty-store
   first-save is guarded (`persistence.rs::has_changed` requires
   non-empty), but a **pruned non-empty** list written by a fresh
   instance holding an early/partial manifest is not — a latent path
   for turning the display race into real disk loss.
3. **F3 — volatile-title restore fallback:** restore matching prefers
   the stable pane id and falls back to `(tab_name, pane_title)`
   (`harpoon-core/src/restore.rs:94-107`). After any zellij session
   restart all pane ids reset, making the fallback the only path — and
   pi panes retitle themselves continuously, so the fallback misses
   panes that are still alive.

Fix mechanism (source-verified against zellij 0.44.3 and empirically
probed 2026-07-13, evidence at `/tmp/spike-handoff/`): **targeted
bootstrap hand-off**. The outgoing instance already knows everything
the successor needs; hand it over directly instead of racing the disk:

- `open_plugin_pane_floating` **returns the new pane id**
  (`zellij-tile/src/shim.rs` → `Option<PaneId>`; server fills
  `affected_pane_id` after a blocking `apply_action!`,
  `zellij_exports.rs:1141`). The current Respawn branch discards this
  return value.
- `MessageToPlugin` supports **`destination_plugin_id` routing**
  (`zellij-utils/src/data.rs:2803`, handler
  `zellij-server/src/plugins/mod.rs:1051`) — delivers by plugin id,
  bypassing url+config instance matching entirely.
- Respawn branch becomes: spawn → capture id → send `bootstrap_store`
  pipe with payload = serialized `BookmarkStore` + session name →
  `close_self()`. Successor adopts the payload instantly; the existing
  disk load remains as cold-boot fallback and reconciliation input.

Probe results (throwaway spike plugin, scratch session):
`spawn_returned Some(Plugin(5))`; `BOOTSTRAP_RECEIVED
payload="store-from-4" source=Plugin(4)` delivered 119ms after
successor load (queued across the loading window); successor event
order was load → first `PaneUpdate` → **bootstrap** → permission
grant. Two probe riders that MUST shape the design: (a) the bootstrap
arrives **pre-grant** — the adoption handler must be deny-safe, pure
state only; (b) the same in-flight invocation pipe was **re-delivered
to the successor** ~380ms later — a stale toggle right after respawn
must not immediately hide the just-shown menu.

User-observable outcome: invoking harpoon from any tab SHALL present
the full persisted jump-target list immediately — no empty-list
flashes, no invoke-again-to-recover — and persistence on disk SHALL
never shrink as a side effect of an instance that has not yet observed
both a resolved disk load and a full pane manifest.

## Decision record (alternatives rejected in explore)

- **Config-embedded state hand-off — FORBIDDEN:** pipe routing matches
  instances by exact `(location, configuration)`
  (`wasm_bridge.rs:1668`); a successor spawned with state baked into
  its configuration no longer matches the keybind's `MessagePlugin`
  config, so every subsequent `Ctrl+y` launches another instance —
  reintroducing the pane pile-up.
- **Url-routed bootstrap (`MessageToPlugin` by url+config):**
  `message_to_plugin` silently injects `caller_cwd` into the config
  before matching (`zellij_exports.rs:818`) and would also deliver the
  bootstrap back to the sender. Destination-id routing avoids both.
- **Synchronous save-before-spawn:** pointless — saves already happen
  at mutation time; disk was verified current. The race is successor
  load latency, not stale disk.
- **Render-gate only (hold menu until load resolves):** adds visible
  latency to every cross-tab invoke and keeps the race machinery;
  acceptable only as a degraded fallback path, not the mechanism.
- **Reopen the respawn ruling (warm cross-tab `show_self`):** would
  contradict the standing owner ruling from pipe-toggle-invocation
  (menu opens on the invoking tab). Not reopened.

## Constraints

- **New permission:** the bootstrap send requires
  `PermissionType::MessageAndLaunchOtherPlugins`
  (`zellij_exports.rs:5435`). Add to `request_permission`, the
  regression/probe script permission seeds, and README (runtime
  regrant step documented). Denied or absent grant MUST degrade to
  today's behavior (disk-load fallback) — never panic (established
  invariant: response-decoding host calls panic on permission-denied
  and must be grant-gated).
- **Deny-safe adoption:** the `bootstrap_store` pipe handler runs
  pre-grant (probe-proven). It MUST mutate pure state only — no
  response-decoding host calls.
- **Core decides, shim executes (Constitution I):** adopt-vs-disk
  precedence (bootstrap payload wins while the disk load is
  unresolved; a late disk result reconciles and never clobbers newer
  in-memory mutations), duplicate-toggle tolerance, and the
  destructive-save guard are pure decision logic in `harpoon-core`
  with native tests; the shim only wires host calls.
- **Duplicate-delivery tolerance:** a queued/stale invocation pipe
  re-delivered to the successor immediately after respawn MUST NOT
  hide the just-shown menu (probe rider b). Covered by a committed
  regression scenario.
- **F2 prune-guard:** bookmark-removing (shrinking) saves are
  forbidden until the instance has observed BOTH a resolved disk load
  AND a full pane manifest. Additive saves remain allowed.
- **F3 title-fallback hardening:** reduce restore dependence on
  volatile `pane_title` matching (pi panes retitle continuously; ids
  reset on session restart). Exact mechanism is a design decision, but
  it MUST respect the existing `reorder` spec restore semantics
  (restore-freeze on user mutation, placeholder slots, non-unique
  identity best-effort) and MUST NOT regress them.
- **Every failure mode degrades to status quo:** spawn returning
  `None`/non-plugin id, bootstrap lost, permission denied — all fall
  back to the existing disk-load path; behavior is never worse than
  today.
- **Spec deltas:** `pane-pipe-api` — respawn state hand-off
  requirement (amending "Toggle Pipe Invocation") + duplicate-delivery
  tolerance scenario; `reorder` — persistence deltas for the
  prune-guard and restore-fallback hardening.
- **Regression evidence:** extend
  `scripts/toggle-pipe-regression.sh` (tmux-hosted scratch sessions
  ONLY — never the user's live workspace): cross-tab respawn presents
  the persisted targets immediately; duplicate-pipe tolerance;
  prune-guard (disk file never shrinks under an early-save window);
  permission-denied degrade. Native `harpoon-core` tests cover the new
  decision logic.
- Build target wasm32-wasip1 only (Constitution III).
- Runtime activation is operational, OUTSIDE gate assertions, but MUST
  be documented: deploy wasm; answer the new
  `MessageAndLaunchOtherPlugins` prompt in a visible pane; verify a
  cross-tab invoke shows the full list instantly.

## Invariants honored

- Constitution I: decision logic (adoption precedence, duplicate
  tolerance, save guard, restore matching) in `harpoon-core`, natively
  tested; shim executes host calls only.
- Constitution II: behavior lands as spec deltas alongside code.
- Constitution III: wasm32-wasip1 is the only build target.
- Constitution IV: never act on unverified/cached host state — the
  hand-off payload is sender-verified live state at send time, and the
  successor treats it as bootstrap data (reconciled against sync
  queries/manifest), not as a substitute for decision-time queries.
