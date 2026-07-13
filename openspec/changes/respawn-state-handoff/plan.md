# Plan — respawn-state-handoff

Execution driver for openspec-apply-change. Worktree-always: create
`/Volumes/Workshop/git/harpoon--opsx-respawn-state-handoff` on branch
`opsx/respawn-state-handoff` at apply start; confirm Diff Base SHA in
review.md equals the merge-base before the first implementation commit.

## Step 1 — Evidence preservation

- **Covers:** T1.1
- **Pre-conditions:** `/tmp/spike-handoff/` still present (src/main.rs,
  probe.sh); zellij log excerpt retrievable. If `/tmp` already purged,
  record the loss in review.md Execution Notes and cite the intent's probe
  summary instead — do NOT re-run the probe against the live workspace.
- **Action:** copy spike source + probe script + SPIKEHO log lines into
  `openspec/changes/respawn-state-handoff/evidence/`; commit.
- **Verification:** files exist in change dir; `openspec validate` still
  green.
- **Rollback:** delete evidence dir; revert commit.

## Step 2 — Core: adoption + precedence (T2.1)

- **Covers:** T2.1
- **Pre-conditions:** Step 1 done (evidence citable).
- **Action (5-step micro-tasks):**
  1. Failing native tests citing `pane-pipe-api.respawn-state-hand-off`:
     adopt-before-disk wins; late disk reconciles without clobbering a
     newer mutation; no-bootstrap cold boot = existing disk path; payload
     round-trips the persistence v2 envelope + session name.
  2. `cargo test -p harpoon-core` → expect FAIL.
  3. Minimal impl in `harpoon-core` (serializer + adoption/reconcile
     decision, no zellij types in the decision surface).
  4. `cargo test -p harpoon-core` → PASS.
  5. Commit (≤72-char subject).
- **Verification:** native suite green; no shim changes in this step.
- **Rollback:** `git revert` the step commit.

## Step 3 — Core: duplicate-toggle pipe identity (T2.2)

- **Covers:** T2.2
- **Action:** same 5-step cycle; tests cite
  `pane-pipe-api.duplicate-toggle-delivery-tolerance` — hand-off payload
  carries the sender-handled CLI pipe id; successor ignores exactly one
  toggle from that source while still releasing the client; keybind or new
  CLI sources are honored (hide branch reachable). Pure identity predicate,
  no timers/readiness proxy (probe: stale delivery arrives after show).
- **Verification / Rollback:** as Step 2.

## Step 4 — Core: save guard + restore hardening (T2.3, T2.4)

- **Covers:** T2.3, T2.4
- **Action:** two 5-step cycles. Guard tests cite
  `reorder.destructive-save-guard` (core owns complete save policy; suppress
  shrink + preserve in-memory bookmarks before readiness; full manifest =
  every known tab; queue unknown-baseline flush; deferred disappeared-pane
  prune resumes once ready). Restore tests cite
  `reorder.restore-identity-tracks-live-panes` (same-session id-carry through
  hand-off; clear generation-untrusted disk ids before fallback/merge/shrink;
  title-drift refresh emits a save; freeze/placeholder/best-effort
  scenarios from the existing reorder suite stay green — run them
  explicitly).
- **Verification:** full `cargo test -p harpoon-core` green (existing
  restore/persistence tests are the non-regression harness).
- **Rollback:** revert the step commits.

## Step 5 — Shim wiring (T3.1–T3.4)

- **Covers:** T3.1, T3.2, T3.3, T3.4
- **Pre-conditions:** Steps 2–4 merged in worktree; core API surface final.
- **Action:** wire spawn-id capture + grant-gated `bootstrap_store` send +
  `close_self` ordering; deny-safe adoption handler (pure state, pre-grant
  tolerant); `MessageAndLaunchOtherPlugins` in `request_permission` and both
  scripts' permission seeds; save-guard + title-refresh wiring. Shim carries
  NO decision logic (Constitution I) — every branch keyed on a core-returned
  decision. Established invariants: response-decoding host calls only
  post-grant (panic otherwise); suppressed panes receive no events.
- **Verification:** `cargo build --target wasm32-wasip1 --release` clean;
  native suite untouched-green.
- **Rollback:** revert shim commits; core remains valid standalone.

## Step 6 — Regression + docs (T4.1, T4.2)

- **Covers:** T4.1, T4.2
- **Pre-conditions:** wasm builds; tmux available; scratch sessions only
  (NEVER the user's live workspace session — standing rule).
- **Action:** add scenarios (instant targets on cross-tab respawn;
  stale-toggle tolerance; prune-guard disk-shrink window;
  aggregate-permission-denied no-panic + terminal/plugin survival) to
  `scripts/toggle-pipe-regression.sh`, plus deterministic core
  instrumentation for full first-render projection, partial-manifest,
  deferred-prune, and stale-id-collision host states; README
  permission + regrant + hand-off notes.
- **Verification:** full regression script pass (existing 11 + new
  scenarios); record counts in review.md Execution Notes.
- **Rollback:** revert; scenarios are additive.

## Step 7 — Validation wrap (T5.1)

- **Covers:** T5.1
- **Action:** `cargo test -p harpoon-core`; wasm release build;
  `openspec validate respawn-state-handoff --strict`; full regression run;
  append evidence lines to review.md Execution Notes.
- **Verification:** all four green — this is the code-review dispatch
  precondition.
- **Rollback:** n/a (verification only).

---

## Analyze (deterministic checks — plain-M inline record)

Run 2026-07-13 by the orchestrator (no blind dispatch at plain M):

1. **Tiling (every delta AC → task):**
   `pane-pipe-api.respawn-state-hand-off` → T2.1, T3.1, T3.2, T4.1(a);
   `pane-pipe-api.duplicate-toggle-delivery-tolerance` → T2.2, T3.2,
   T4.1(b); `pane-pipe-api.host-call-permission-completeness` → T3.3,
   T4.1(d); `reorder.destructive-save-guard` → T2.3, T3.4, T4.1(c);
   `reorder.restore-identity-tracks-live-panes` → T2.4, T3.4. **No orphan
   ACs; no task without an AC or infra/doc purpose. PASS.**
2. **EARS lint (error paths use IF…THEN, never WHEN):** all
   unwanted-condition scenarios in both delta specs use `IF`/`THEN`
   (spawn-id unavailable; bootstrap send denied; ungranted permission).
   Nominal paths use WHEN/WHILE. **PASS.**
3. **Traceability (AC IDs greppable):** canonical IDs listed above appear
   verbatim in tasks.md/plan.md and are mandated as test citations. **PASS.**
7. **Intent alignment:** mechanism, constraints (deny-safe pre-grant
   adoption, duplicate tolerance, degrade-to-status-quo, FORBIDDEN
   config-embedded hand-off), and non-goals match frozen intent.md
   (commit `7409760b`) — no rejected alternative reappears. **PASS.**

Blockers: none.
