# Code Review

**Change:** respawn-state-handoff
**Verdict:** pass
**review_mode:** disclosure-consensus
**reviewer-provenance:** openai-codex/gpt-5.6-sol + claude-bridge/claude-opus-4-8
**Diff Base SHA:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4
**Reviewed Range:** 887e8faaacc3ad77b7c91116f60976c3360b4ce4..155bb7a6201d60d64e99eaad66609a8f2581c010
**Attested HEAD:** 155bb7a6201d60d64e99eaad66609a8f2581c010
**Baseline:** frozen intent.md + proposal + specs + plan + tasks status + constitution/domain (design absent by plain-M decision gate)
**Generated:** 2026-07-13

## Verdict contract

FAIL only for a frozen-baseline violation or objective correctness/security
defect. P0/P1 gate; P2/P3 are advisory. Verdict pass iff no P0/P1 remains
open.

## Round tracker

| Round | Mode | P0 | P1 | P2 | P3 | Reviewer verdicts | Reviewed HEAD |
|---|---|---:|---:|---:|---:|---|---|
| 1 | blind | 0 | 7 | 3 | 4 | gpt-5.6-sol:fail claude-fable-5:fail | d731bf6e4fcfabdee9bac01511afd01ff5a49c3a |
| 2 | blind | 2 | 5 | 3 | 3 | gpt-5.6-sol:fail claude-fable-5:fail | 5e6692c145021304959a02b6fb75a4efebc1ee2b |
| 3 | blind | 1 | 4 | 0 | 1 | gpt-5.6-sol:fail claude-opus-4-8:fail (Fable capacity replacement) | 8fc491122b9f1d568ff7feb681417b2ac70d248f |
| 4 | blind | 0 | 3 | 1 | 1 | gpt-5.6-sol:fail claude-opus-4-8:pass (Fable capacity replacement) | 81639347def2c1121d2e4a3791a8db8534cf3e73 |
| 5 | blind | 0 | 3 | 0 | 2 | gpt-5.6-sol:fail claude-opus-4-8:pass (Fable capacity replacement) | db48288dd0cc8080e9a01e57a9ca9f092accf01d |
| disclosure | disclosure-consensus | 0 | 0 | 0 | 1 | gpt-5.6-sol:pass claude-opus-4-8:pass | 155bb7a6201d60d64e99eaad66609a8f2581c010 |

<!-- Sole-source disclosure findings files:
/tmp/rsh-disclosure-sol.md and /tmp/rsh-disclosure-opus.md.
Configured Fable exhausted provider capacity; unchanged-tree Claude Opus
replacement participated from round 3 onward. Invalid no-file/non-template
attempts were excluded from counts. -->

## Findings

| # | Finding | Severity | Status |
|---|---|---|---|
| joint-1 | S10's 5ms valid-JSON monitor is sampling-based and could miss a sub-poll transient shrink; deterministic core guard/race tests and before/after assertions are authoritative coverage. | P3 | open advisory |

## Disclosure reconciliation

- Round-5 exact-id theft P1: **resolved** by `11bc19d`; restore now reserves
  every visible trusted exact-id claim globally before fallback, with the
  staggered pane-11-before-pane-10 regression.
- Round-5 v1 provenance P1: **resolved** by `11bc19d`; parse provenance flows
  through MergeMissing/ReconcileBaseline and forces the next v2 save.
- Round-5 delayed S8 observation P1: **resolved** by `11bc19d`; raw F6 send
  bypasses the helper delay and polling begins immediately with disk absent.
- Both disclosure participants independently ratified the final joint set.

## Applied fixes

- `3708e68`: full projection, generation-scoped identity, core save policy,
  deferred prune, denial safety.
- `9bda01a`: multiset identity, no-index rows, adopted-id prune enrollment,
  composed race model.
- `1d08994`: cross-round consumption, v1 migration state, total corrupt-index
  projection, disk-absent S8, continuously monitored S10.
- `11bc19d`: global exact-id reservation, reconcile v1 provenance, immediate
  S8 observation.

## Residual risks

- `joint-1` is advisory only and does not undermine deterministic core proof.
- Runtime deployment/regrant remains outside this gate per frozen intent.

## Verdict rationale

PASS. Disclosure consensus resolved every open P0/P1 from the five blind
rounds. Final joint set has P0=0/P1=0; both distinct reviewer models attested
current HEAD and returned pass. Core tests 269/269, wasm32-wasip1 release,
strict OpenSpec, and scratch regression 26/26 are green. No gate or validation
manifest changed.
