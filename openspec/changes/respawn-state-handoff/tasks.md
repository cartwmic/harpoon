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
- [x] 2.2 Duplicate-toggle readiness condition: deterministic state
  condition (bootstrap-or-load resolved AND first shown render complete)
  under which a `toggle` is honored; stale re-delivered toggles before
  readiness are ignored, never wall-clock debounced. Tests cite
  `pane-pipe-api.duplicate-toggle-delivery-tolerance`.
  - intent: feature
  - files_allowed:
      - harpoon-core/**
- [x] 2.3 Destructive save guard: pure decision forbidding shrinking saves
  until BOTH a resolved disk load AND a full pane manifest have been
  observed; additive/reordering saves always allowed; pruning resumes after
  both observed. Tests cite `reorder.destructive-save-guard`.
  - intent: fix
  - files_allowed:
      - harpoon-core/**
- [x] 2.4 Restore identity hardening: id-first resolution carries through
  the hand-off payload (successor resolves by id without title matching);
  resolved bookmarks refresh persisted `(tab_name, pane_title)` on observed
  drift; freeze/placeholder/best-effort semantics regress nothing. Tests
  cite `reorder.restore-identity-tracks-live-panes`.
  - intent: fix
  - files_allowed:
      - harpoon-core/**

## 3. Shim wiring (harpoon-plugin)

- [ ] 3.1 Respawn hand-off send: capture `open_plugin_pane_floating`'s
  returned pane id, send `bootstrap_store` via `pipe_message_to_plugin`
  with `destination_plugin_id` (payload from core serializer), then
  `close_self()`; grant-gate the send; `None`/non-plugin id or
  denied/ungranted permission skips the hand-off and degrades to the
  disk-load path (never panic).
  - intent: feature
  - files_allowed:
      - harpoon-plugin/**
- [ ] 3.2 `bootstrap_store` pipe handler: deny-safe, pure-state adoption
  (no response-decoding host calls — payload can arrive pre-grant), feeding
  core's adoption logic; wire duplicate-toggle readiness into the toggle
  pipe path.
  - intent: feature
  - files_allowed:
      - harpoon-plugin/**
- [ ] 3.3 Permission completeness: add `MessageAndLaunchOtherPlugins` to
  `request_permission`; update both scripts' permission seeds. Tests/greps
  cite `pane-pipe-api.host-call-permission-completeness`.
  - intent: feature
  - files_allowed:
      - harpoon-plugin/**
      - scripts/**
- [ ] 3.4 Wire save guard + title-drift refresh into the
  `update_panes`/persistence path (shim executes core decisions only).
  - intent: fix
  - files_allowed:
      - harpoon-plugin/**

## 4. Regression evidence + docs

- [ ] 4.1 Extend `scripts/toggle-pipe-regression.sh` (tmux-hosted scratch
  sessions ONLY): (a) cross-tab respawn presents persisted targets on first
  render; (b) stale re-delivered toggle does not hide the menu; (c)
  prune-guard — disk file never shrinks during the early-save window; (d)
  permission-denied degrade (no panic, disk-load fallback).
  - intent: feature
  - files_allowed:
      - scripts/**
- [ ] 4.2 README: document the new `MessageAndLaunchOtherPlugins`
  permission and the runtime regrant step (visible-pane prompt), plus the
  hand-off behavior note.
  - intent: feature
  - files_allowed:
      - README.md

## 5. Validation

- [ ] 5.1 Full validation: `cargo test -p harpoon-core` green; wasm build
  (`wasm32-wasip1`) clean; `openspec validate respawn-state-handoff
  --strict` passes; full regression script run recorded in review.md
  Execution Notes.
  - intent: infra
  - files_allowed:
      - openspec/changes/respawn-state-handoff/**
