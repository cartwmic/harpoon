## 1. Evidence preservation

- [x] 1.1 Preserve the 2026-07-13 spike-handoff probe evidence (spike plugin
  `src/main.rs`, `probe.sh`, and the SPIKEHO log excerpt proving spawn-id
  return, 119ms targeted bootstrap delivery, pre-grant arrival, and ~380ms
  stale-pipe re-delivery) under
  `openspec/changes/respawn-state-handoff/evidence/` — `/tmp` copies do not
  survive reboot and the frozen intent cites them.
  - intent: infra
  - files_allowed:
      - openspec/changes/respawn-state-handoff/**

## 2. Core decision logic (harpoon-core, native tests)

- [x] 2.1 Bootstrap adoption + precedence: pure core logic deciding how an
  adopted `bootstrap_store` payload (v2 envelope + session name) merges with
  a disk-load result — adopted payload wins while the disk load is
  unresolved; a late disk result reconciles and never clobbers newer
  in-memory mutations; cold boot without bootstrap keeps today's disk path.
  Tests cite `pane-pipe-api.respawn-state-hand-off`.
  - intent: feature
  - files_allowed:
      - harpoon-core/**
- [x] 2.2 Duplicate-toggle pipe-identity guard: the hand-off payload carries
  the sender-handled CLI pipe id; the successor ignores exactly one toggle
  from that same source while still releasing the client. Keybind/different
  CLI sources are honored; never wall-clock/readiness debounced. Tests cite
  `pane-pipe-api.duplicate-toggle-delivery-tolerance`.
  - intent: feature
  - files_allowed:
      - harpoon-core/**
- [x] 2.3 Destructive save guard: pure core decisions own complete save
  policy, forbid shrinking saves and preserve bookmarks in memory until
  BOTH resolved disk + full manifest (every known tab); queue unknown-
  baseline flush; and release deferred disappeared/adopted-absent ids once
  ready. Duplicate identities use one-to-one multiplicity. Tests cite
  `reorder.destructive-save-guard`.
  - intent: fix
  - files_allowed:
      - harpoon-core/**
- [x] 2.4 Restore identity hardening: same-session id-first resolution
  carries through hand-off; disk-parsed ids clear as generation-untrusted
  before fallback restore/merge/shrink comparison; resolved bookmarks
  refresh persisted `(tab_name, pane_title)` on observed drift; freeze/
  placeholder/best-effort semantics regress nothing. Tests
  cite `reorder.restore-identity-tracks-live-panes`.
  - intent: fix
  - files_allowed:
      - harpoon-core/**

## 3. Shim wiring (harpoon-plugin)

- [x] 3.1 Respawn hand-off send: capture `open_plugin_pane_floating`'s
  returned pane id, send `bootstrap_store` via `pipe_message_to_plugin`
  with `destination_plugin_id` (payload from core serializer), then
  `close_self()`; grant-gate the send; `None`/non-plugin id or
  unavailable payload after grant skips hand-off and degrades to successor
  disk load; aggregate permission denial stays deny-safe/show-in-place
  (never panic).
  - intent: feature
  - files_allowed:
      - harpoon-plugin/**
- [x] 3.2 `bootstrap_store` pipe handler: deny-safe, pure-state adoption
  (no response-decoding host calls — payload can arrive pre-grant), feeding
  core's adoption logic; wire same-pipe-id one-shot duplicate tolerance into
  the toggle pipe path.
  - intent: feature
  - files_allowed:
      - harpoon-plugin/**
- [x] 3.3 Permission completeness: add `MessageAndLaunchOtherPlugins` to
  `request_permission`; update both scripts' permission seeds. Tests/greps
  cite `pane-pipe-api.host-call-permission-completeness`.
  - intent: feature
  - files_allowed:
      - harpoon-plugin/**
      - scripts/**
- [x] 3.4 Wire save guard + title-drift refresh into the
  `update_panes`/persistence path (shim executes core decisions only).
  - intent: fix
  - files_allowed:
      - harpoon-plugin/**

## 4. Regression evidence + docs

- [x] 4.1 Extend `scripts/toggle-pipe-regression.sh` (tmux-hosted scratch
  sessions ONLY): (a) cross-tab respawn presents persisted targets on first
  render (plus deterministic full race sequence covering no-index projection,
  partial guard, and ready prune); (b) stale
  re-delivered toggle does not hide the menu; (c) prune-guard — disk file
  never shrinks during early-save window (plus deterministic partial-
  manifest, deferred-prune, and stale-id-collision instrumentation); (d)
  aggregate permission denial is inert/deny-safe (no panic, session/plugin
  survive while terminal remains visible; no impossible post-denial show).
  - intent: feature
  - files_allowed:
      - scripts/**
- [x] 4.2 README: document the new `MessageAndLaunchOtherPlugins`
  permission and the runtime regrant step (visible-pane prompt), plus the
  hand-off behavior note.
  - intent: feature
  - files_allowed:
      - README.md

## 5. Validation

- [x] 5.1 Full validation: `cargo test -p harpoon-core` green; wasm build
  (`wasm32-wasip1`) clean; `openspec validate respawn-state-handoff
  --strict` passes; full regression script run recorded in review.md
  Execution Notes.
  - intent: infra
  - files_allowed:
      - openspec/changes/respawn-state-handoff/**
