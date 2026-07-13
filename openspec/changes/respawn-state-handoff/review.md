---
# Machine-readable mode block — the SOLE source opsx gate reads (it never parses
# the prose table below). Keep the table in sync as the human-facing mirror.
scale: M
full_rigor: false
execution_mode: standard
verification_mode: retained-recommended
debug_mode: standard
review_status: not-requested
delegation_mode: single-agent
# code_review_mode: derived when absent (M ⇒ gating-required)
loop_max_iterations: 40
validation_source_mode: required
spec_level: spec-anchored
doneness_mode: required
---

<!-- authored: in-session -->

# Review

## Modes

| Mode | Value | Notes |
|---|---|---|
| Scale | M | Frozen intent.md recommends Scale M, `full_rigor: false` — three related defects (F1 race, F2 prune-guard, F3 restore hardening), two capability deltas, core+shim+script+README surface; owner confirmed M at explore wrap |
| full_rigor | false | Per intent recommendation and owner pick — no cross-capability breakage/migration |
| Execution Mode | standard | |
| Verification Mode | retained-recommended | |
| Debug Mode | standard | |
| Review Status | not-requested | |
| Delegation Mode | single-agent | |
| Code Review Mode | derived (absent) | M ⇒ gating-required (derived fail-closed) |
| Loop Max Iterations | 40 | Authoring-time default for M |
| Validation Source Mode | required | Committed tmux-hosted regression script `scripts/toggle-pipe-regression.sh` (extended scenarios) is the agent-independent validation source (precedent: pipe-toggle-invocation) |
| Doneness Mode | required | Plain-M ⇒ doneness rides the code-review dispatch (designated reviewer) |
| Spec Level | spec-anchored | Default; owner pick at explore wrap |
| Model Config | (unset) | Roles resolve via `opsx models`; session model fallback |

## Diff Base + Worktree locator

**Diff Base SHA:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4
**Worktree Path:** /Volumes/Workshop/git/harpoon--opsx-respawn-state-handoff
**Integration Branch:** main

<!-- Diff Base confirmed at worktree creation = main HEAD (propose commit);
immutable from here. -->

## Manual Adjustments

- Scale M / full_rigor false / spec-anchored adopted directly from frozen
  intent.md and the owner's explicit picks at the explore→propose handoff —
  no deviation.
- design.md deliberately NOT authored (plain-M decision-gated): every
  non-trivial trade-off (hand-off mechanism, rejected alternatives, Q1–Q3)
  is already settled in the FROZEN intent.md Decision record and
  proposal.md ## Open Questions; a design.md would restate frozen content
  and add a fidelity dispatch with no new decision surface.

## Scope Expansions

### SE-1 — aggregate permission denial replaces hand-off-only denial fallback

- **Trigger:** blind code-review round 1 P1 (both reviewers) required a
  permission-denied regression and a spawn-without-hand-off fallback.
- **Evidence:** zellij 0.44.3 `request_permission` returns only
  `Event::PermissionRequestResult(status)` — the event carries no permission
  identity — and caches/blocks ALL normal plugin events while any permission
  request is unresolved (`zellij-server/src/plugins/zellij_exports.rs:892`,
  `plugins/mod.rs:767`). Runtime probe 2026-07-13: splitting baseline and
  `MessageAndLaunchOtherPlugins` into sequential/nested requests delivered
  only the first Granted result and left the plugin permission-modal; S2
  toggle and Esc never reached harpoon. Two load-time requests behaved the
  same. Therefore "OpenTerminalsOrPlugins granted while only
  MessageAndLaunchOtherPlugins denied" is not representable through the SDK.
- **Substitution:** request the complete capability vector atomically. On
  aggregate grant, spawn+targeted hand-off run. On aggregate denial, issue NO
  gated response-decoding/query/output/unblock call and use deny-safe
  show-in-place (no panic). Payload encode/send-unavailable AFTER aggregate
  grant still degrades spawn→no hand-off→successor independent disk load.
  S11 drives the real interactive denial in a scratch session and asserts
  pane/session survival + zero new panic lines.
- **Why required:** preserves the frozen intent's safety outcome (never panic,
  never assume unverified host capability) using the only host-observable
  permission state; exact hand-off-only denial branch is host-unrepresentable.

### SE-2 — full-manifest readiness made verifiable

- **Trigger:** round-1 P1/P3 found the old non-empty-manifest proxy reopened
  the partial-manifest prune hole.
- **Evidence/substitution:** zellij exposes PaneUpdate snapshots but no
  explicit completeness bit. Define "full" as manifest coverage of EVERY tab
  position currently known from TabUpdate (pure core predicate, native test);
  preserve bookmarks in memory as well as suppress disk writes until BOTH
  that coverage and independent disk resolution hold.

### SE-3 — pane-id trust is scoped to one zellij session generation

- **Trigger:** blind code-review round 2 P0/P1 found reused persisted pane ids
  could defeat shrink detection and bind cross-restart restore to an unrelated
  pane before fallback.
- **Evidence:** `PaneBookmark` already documents id as session-local/not
  globally unique; the frozen F3 states ids reset on restart. Persistence v2
  carries no session-generation token, so equality of a disk id with a live
  id cannot prove identity. Targeted bootstrap differs: predecessor and
  successor coexist in the SAME generation, so carried ids are trustworthy.
- **Substitution:** clear pane ids parsed from disk before cold restore,
  MergeMissing, or persistence-baseline comparison; compare mixed current-id/
  no-id rows by refreshed fallback identity. Preserve id-first behavior for
  targeted bootstrap and already-resolved live memory. This narrows the
  original unqualified "id-first" wording to the only provable trust domain
  and directly enforces F3's restart fallback outcome.

## Execution Notes

<!-- Transient observations appended during apply. One-line entries when a
non-trivial decision is made mid-task. Durable knowledge → retrospective.md. -->

- 2026-07-13 — Task 1.1: spike evidence preserved from /tmp into
  evidence/ (log excerpt confirms spawn-id return, 119ms pre-grant
  bootstrap delivery, ~380ms same-uuid CLI pipe re-delivery).
- 2026-07-13 — Tasks 2.1–2.4 (core, 259 native tests green): duplicate
  tolerance mechanism CORRECTED from the propose-time sketch — probe
  timing proves the re-delivered pipe arrives AFTER the menu is shown, so
  the spec's "readiness after first shown render" parenthetical could not
  catch it; amended spec/proposal (Q3) to deterministic pipe-identity
  matching (payload carries sender-handled CLI pipe id, one-shot ignore,
  client still released). Keybind-sourced pipes carry no id and are never
  suppressed (re-delivery is a CLI-pipe blocking-mechanism behavior).
- 2026-07-13 — Assumption clarified after round-1: adopted bootstrap may be
  used as a comparison baseline for proving a save additive/reorder-only,
  but NEVER substitutes for independent `disk_resolved` in the shrinking-
  save readiness predicate. Adopted late disk result reconciles by replacing
  the comparison baseline only; memory remains verbatim (no stale-row merge).
  No-bootstrap disk reconcile after early user mutation = MergeMissing (disk
  entries absent from memory append with index=None), never replace memory.
- 2026-07-13 — Apply validation at worktree `d731bf6`: core native tests
  259/259 green; wasm32-wasip1 release build clean; strict OpenSpec valid;
  shell syntax clean; expanded tmux-hosted scratch regression 19/19 green
  (S8 immediate target hand-off, S9 same-uuid duplicate tolerance + CLI
  release, S10 persisted-list non-shrink across respawns).
- 2026-07-13 — Blind code-review round 1 at worktree `d731bf6`: both fail;
  max counts P0=0/P1=7/P2=3/P3=4; designated doneness judge = not. Fix commit
  `5893b0e` addresses all P1s: suppress interim rendering + immediate cached
  restore, independent exactly-once disk initiation + baseline reconcile,
  independently-resolved-disk/full-manifest prune readiness + in-memory
  preservation, pre-grant CLI queue, aggregate permission denial safety,
  spawn-id-unavailable close, and deterministic/interactive evidence.
  Post-fix regression 23/23: adds S0 render/full-manifest instrumentation and
  S11 real permission prompt denial (pane/session alive, no new panic).
- 2026-07-13 — Round-2 candidate worktree `5e6692c` revalidated: core
  261/261, wasm32-wasip1 release, strict OpenSpec, shell syntax, regression
  23/23 all green. Runtime permission experiment also established zellij's
  aggregate result semantics and the need to queue pre-grant CLI pipes:
  without queueing, S7 cold jump was dropped; with queue+post-grant drain,
  S7 passes and every drained CLI client follows the normal exactly-once
  release path.
- 2026-07-13 — Blind code-review round 2 at worktree `5e6692c`: both valid
  reviewers fail; max P0=2/P1=5/P2=3/P3=3; designated doneness = not. One
  initial fable dispatch timed out without a findings file and was INVALID;
  same blind round re-dispatched with unchanged dual-tree snapshots.
  Fix `3708e68`: full-row first-render predicate + saved-identity placeholder
  text; generation-untrusted disk-id clearing; mixed id/no-id reconciliation;
  v1 known baseline; complete core-owned save decision; deferred prune queue
  released after readiness; denial renders empty UI; plugin-only bootstrap;
  cached-grant bootstrap starts disk reconcile. Core 264/264, wasm clean,
  expanded scratch regression 25/25.

## Fidelity Round Ledger

| Round | Fidelity | Per-judge verdicts | Attested HEAD |
|---|---|---|---|

<!-- Design-bearing changes only; this change carries no design.md, so this
ledger stays empty unless a design.md is later authored under a re-ruling. -->

## Code Review Round Ledger

| Round | Verdict | P0 | P1 | Reviewers | Attested HEAD |
|---|---|---|---|---|---|
