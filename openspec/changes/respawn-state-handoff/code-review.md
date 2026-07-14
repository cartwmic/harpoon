# Code Review

**Change:** respawn-state-handoff
**Verdict:** fail
**review_mode:** adversarial-multimodel
**reviewer-provenance:** openai-codex/gpt-5.6-sol + claude-bridge/claude-fable-5 (blind via pi-subagents delegate)
**Diff Base SHA:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4
**Reviewed Range:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4..8fc491122b9f1d568ff7feb681417b2ac70d248f
**Attested HEAD:** 8fc491122b9f1d568ff7feb681417b2ac70d248f
**Baseline:** intent.md + proposal + specs + plan + tasks status + constitution/domain (design absent by plain-M decision gate)
**Generated:** 2026-07-13

## Verdict contract (embed in every reviewer dispatch prompt)

FAIL only for a frozen-baseline violation or objective correctness/security
defect. P0/P1 gate; P2/P3 are advisory. Verdict pass iff no P0/P1 remains
open.

## Round tracker

| Round | Mode | P0 | P1 | P2 | P3 | Reviewer verdicts | Reviewed HEAD |
|---|---|---|---|---|---|---|---|
| 1 | blind | 0 | 7 | 3 | 4 | gpt-5.6-sol:fail claude-fable-5:fail | d731bf6e4fcfabdee9bac01511afd01ff5a49c3a |
| 2 | blind | 2 | 5 | 3 | 3 | gpt-5.6-sol:fail claude-fable-5:fail | 5e6692c145021304959a02b6fb75a4efebc1ee2b |
| 3 | blind | 1 | 4 | 0 | 1 | gpt-5.6-sol:fail claude-opus-4-8:fail (Fable capacity replacement) | 8fc491122b9f1d568ff7feb681417b2ac70d248f |

## Findings

<!-- Counts above are max-across-reviewers per severity, with no
cross-reviewer finding matching. Full sole-source findings files:
/tmp/rsh-r1-sol.md and /tmp/rsh-r1-fable.md. Rows retain source identity;
similar findings are not merged for counting. Round-2 sole-source findings:
/tmp/rsh-r2-sol.md and /tmp/rsh-r2-fable-retry.md. Initial fable dispatch
produced no findings file and was INVALID; same blind round was re-dispatched. -->

| # | Finding | Severity | Status |
|---|---|---|---|
| sol-1 | Successor can render before bootstrap, then adoption clears panes without resolving cached manifest — violates first-render full-target guarantee. | P1 | open |
| sol-2 | Bootstrap can prevent disk-load initiation (session name set first), and adopted late disk result is ignored rather than reconciled. | P1 | open |
| sol-3 | Save readiness treats any non-empty pane event + bootstrap adoption as resolved-disk + full-manifest; partial-manifest prune hole remains. | P1 | open |
| sol-4 | Unknown persisted baseline classifies every candidate as shrinking, blocking additive/reorder saves contrary to delta AC. | P1 | open |
| sol-5 | Permissions are tracked as one bool; bootstrap-send denial cannot still take spawn→disk-fallback, and other gated CLI calls remain unguarded. | P1 | open |
| sol-6 | Spawn returning None shows/keeps predecessor instead of the delta scenario's close + successor disk fallback. | P1 | open |
| sol-7 | Regression does not force first-render state, partial-manifest prune window, or permission-denied degradation. | P1 | open |
| fable-1 | Permission-denied degrade regression promised by frozen intent/tasks/plan is omitted while task is checked complete. | P1 | open |
| fable-2 | Adoption skips immediate cached-manifest restore, leaving an empty/resolving render window. | P2 | open |
| fable-3 | Tasks/plan still describe superseded readiness-based duplicate guard; implementation/spec now use pipe identity. | P2 | open |
| fable-4 | Spawn-None delta wording conflicts with status-quo show-in-place behavior; split from non-plugin-id case. | P2 | open |
| fable-5 | Unknown-baseline additive save is deferred; spec currently says all additive saves proceed. | P3 | open |
| fable-6 | MergeMissing can resurrect a user-deleted disk bookmark in the tiny no-bootstrap pre-load window. | P3 | open |
| fable-7 | `manifest_seen` is a non-empty proxy, not a defined full-manifest condition. | P3 | open |
| fable-8 | Bootstrap-before-stale-toggle ordering relies on probe ordering; theoretical reverse order remains. | P3 | open |
| r2-sol-1 | Render readiness is not tied to cached-manifest restoration; bootstrap can still permit empty/partial first render. | P1 | open |
| r2-sol-2 | ID-only identity collisions after restart can bypass merge/shrink detection and overwrite a distinct disk bookmark. | P0 | open |
| r2-sol-3 | Cross-restart reused pane IDs can resolve a bookmark to the wrong pane before title fallback. | P1 | open |
| r2-sol-4 | Mixed id/no-id identities can duplicate the same bookmark during unknown-baseline reconciliation. | P1 | open |
| r2-sol-5 | Successful v1 load leaves no baseline, delaying additive save despite known disk state. | P1 | open |
| r2-sol-6 | Shim decides unknown-baseline + final destructive-save branch instead of core. | P0 | open |
| r2-sol-7 | S8/S10 timing evidence does not force actual first-render and partial-manifest early-save race windows. | P1 | open |
| r2-fable-1 | Aggregate permission denial leaves render permanently suppressed/blank instead of status-quo empty UI. | P1 | open |
| r2-fable-2 | Frozen-store bookmark lost during guard window remains a permanent ghost after readiness; pruning never resumes. | P1 | open |
| r2-fable-3 | Any local/broadcast `bootstrap_store` source can replace memory and seed baseline. | P2 | open |
| r2-fable-4 | Spawn `None` closes only instance if spawn actually failed; status-quo predecessor survived. | P2 | open |
| r2-fable-5 | Bootstrap before first PaneUpdate can still render an empty target list. | P2 | open |
| r2-fable-6 | Post-grant bootstrap session name does not itself trigger exactly-once disk initiation. | P3 | open |
| r2-fable-7 | Aggregate denial leaves unbounded queued CLI clients blocked. | P3 | open |
| r2-fable-8 | Identity refresh clones pane-id map every update round. | P3 | open |
| r3-sol-1 | Identity comparison is not trusted-id-strict or multiplicity-safe for duplicate fallback identities. | P1 | open |
| r3-sol-2 | Unresolved persisted `index=None` bookmark has no projected row, making full-render equality unsatisfiable. | P1 | open |
| r3-sol-3 | Adopted resolved id absent from successor manifest is never enrolled in deferred pruning. | P1 | open |
| r3-sol-4 | Race evidence remains helper-level rather than one deterministic end-to-end state sequence. | P1 | open |
| r3-opus-1 | Unresolved persisted `index=None` bookmark permanently suppresses menu render. | P0 | open |
| r3-opus-2 | Duplicate fallback identities mask multiplicity in shrink/merge. | P3 | open |

## Applied fixes

- Round 1: candidate `5e6692c` added render gating/cached restore, independent
  disk reconciliation, full-manifest/in-memory prune protection, unknown-
  baseline queueing, aggregate permission safety, CLI pre-grant queue, spawn-id
  fallback, and 23-scenario evidence.
- Round 2: candidate `8fc4911` added generation-scoped identity, complete core
  save policy, meaningful full-row projection, deferred prune, v1 baseline,
  denial terminal state, and 25-scenario evidence. Round 3 found gaps above.

## Residual risks

- Native core tests: 259 green. Scenario scripts were not run by reviewers
  (read-only/global-state rule). No gate/validation manifest touched.

## Verdict rationale

FAIL. Valid blind round-3 reviewers found open P0/P1 correctness and evidence
gaps. Configured Fable reviewer was INVALID (provider capacity; no
attestation); unchanged-snapshot Claude Opus replacement supplied second blind
findings file. Fixes must land before full-diff blind round 4.
