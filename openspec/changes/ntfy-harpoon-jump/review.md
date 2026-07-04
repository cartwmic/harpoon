---
# authored: in-session
# Machine-readable mode block — the SOLE source opsx gate reads (it never parses
# the prose table below). Keep the table in sync as the human-facing mirror.
scale: M
# full_rigor: false — single capability (harpoon CLI-pipe interface additions),
# not a breaking change / new capability / cross-capability migration. Plain M.
full_rigor: false
# worktree_mode DERIVED by tier when ABSENT: M ⇒ worktree-required.
execution_mode: standard
verification_mode: retained-recommended
debug_mode: standard
review_status: not-requested
delegation_mode: subagent-eligible
# code_review_mode derived: M ⇒ gating-required.
loop_max_iterations: 40
validation_source_mode: required
spec_level: spec-anchored
doneness_mode: required
review_max_rounds: 5
---

# Review

<!--
Controlled-vocabulary mode switchboard. The apply instruction reads these modes
and dispatches behavior; opsx gate reads the YAML front-matter above. Override
any mode by setting it (in BOTH the front-matter and this table).
-->

## Modes

| Mode | Value | Notes |
|---|---|---|
| Scale | M | XS\|S\|M — typical feature, cross-file but single capability (harpoon CLI-pipe interface) |
| full_rigor | false | false\|true — this change is a single-capability additive feature, not L/XL-class |
| Execution Mode | standard | standard\|tdd-preferred\|tdd-required |
| Verification Mode | retained-recommended | inline-only\|retained-recommended\|retained-required |
| Debug Mode | standard | standard\|systematic-debugging |
| Review Status | not-requested | not-requested\|requested\|findings-received\|resolved |
| Delegation Mode | subagent-eligible | single-agent\|subagent-eligible\|subagent-required |
| Worktree Mode | derived (absent) | M ⇒ worktree-required (tier default) |
| Code Review Mode | derived (absent) | M ⇒ gating-required (tier default); blocks archive on code-review.md Verdict |
| Loop Max Iterations | 40 | iteration budget mapped onto loop runtime turn budget |
| Validation Source Mode | required | opsx-gates.yaml is the agent-independent validation source |
| Doneness Mode | required | gate reads a sealed doneness.md verdict |
| Spec Level | spec-anchored | spec-anchored\|spec-first\|spec-as-source |
| Model Config | (unset) | resolved by `opsx models`; unset ⇒ session model |

## Diff Base + Worktree locator

**Diff Base SHA:** 80627802fc3e6eeda338f6501b861ce5aea22c4c
**Worktree Path:** /Volumes/Workshop/git/harpoon-ntfy-harpoon-jump
**Integration Branch:** main

## Manual Adjustments

- Scale M (not S): additive feature spanning the plugin pipe handler plus new
  pane-id reconciliation logic and tests — cross-file, single capability.
- full_rigor false: no breaking change, no new capability, no cross-capability
  migration; design.md remains decision-gated.

## Execution Notes

- 2026-07-04 — review.md authored in-session; Scale M, worktree-required derived.

## Scope Expansions

- <none yet>
