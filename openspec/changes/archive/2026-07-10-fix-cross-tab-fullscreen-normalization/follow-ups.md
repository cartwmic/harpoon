# Follow-ups

**Change:** fix-cross-tab-fullscreen-normalization
**Created:** 2026-07-09 (first out-of-scope routing)

## Queue

| # | Finding | Severity | Origin (review type, round) | Routing reason | Status |
|---|---|---|---|---|---|
| 1 | jump_pane pipe arriving while the harpoon UI pane is visible+focused resolves Unknown (focused pane != target's terminal) → no toggle, jump lands tiled. Spec-sanctioned carve-out ("no toggle from unknown state"); pre-existing behavior. harpoon-plugin/src/main.rs jump path | P2 | code-review round 1 (fable-5 residual) | Not required for the frozen intent's outcomes (intent scopes notification/keybind jumps; UI-visible pipe is a degenerate self-jump) | open |
| 2 | Harness fullscreen detection keys on the '││' border glyph pair; non-default pane-frame themes could defeat the assertion. scripts/fullscreen-regression.sh fullscreen_showing() | P2 | code-review round 1 | Harness runs in its own isolated session with default config; theme-hardening not required for intent outcomes | open |
| 3 | zellij-tile 0.44.3 names the stable tab id `focused_tab_index` in get_focused_pane_info; a future SDK release changing it to a positional index would silently misquery after tab reorders. Verified correct for 0.44.3 (server: active_tab_ids stores tab.id) | P3 | code-review round 1 (fable-5 residual) | SDK-upgrade hygiene concern, not a defect on the pinned 0.44.3 floor | open |
| 4 | chezmoi ~/bin/zellij-jump: add matching --plugin-configuration so ntfy pipes reach the warm keybind instance (latency optimization; twin instance is reused after first pipe anyway). Lives in the chezmoi repo | P3 | intent Non-goals (explore session) | Cross-repo; explicitly out of scope per frozen intent Non-goals | open |

## Waivers

- (none)

## Promotion

- (filled at archive)
