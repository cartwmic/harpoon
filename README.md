# harpoon-zellij (personal fork)

A [Zellij](https://zellij.dev) plugin for quickly accessing your most-used
panes. Three first-class modes: command (today's bare-key behavior), filter
(type-to-narrow), and jump (slot keys 1-9, a-z).

Forked from [Nacho114/harpoon](https://github.com/Nacho114/harpoon); see
"Custom modifications" below for the divergence from upstream.

![usage](https://github.com/Nacho114/harpoon/raw/main/img/usage.gif)

## Modes

The plugin runs in exactly one of three mutually-exclusive modes at any time.
Default is `command` (matches today's bare-key UX); change via the
`default_mode` config key (see [Configuration](#configuration)).

```
        ┌─ command (home) ─┐
       /                    \
      / Esc                  \ Esc
     ↓                        ↓
 ┌─filter─┐              ┌──jump──┐
 │  /ed   │              │ press  │
 │        │              │ 1-9/a-z│
 └────────┘              └────────┘

 Esc from command  → close plugin
 / from command    → enter filter mode
 # from command    → enter jump mode
 Esc from filter   → clear query, OR drop to command if query empty
 Esc from jump     → drop to command
```

A 1-character mode badge `[N]` / `[F]` / `[J]` appears in every header so you
always know where you are.

### Command mode (`[N]`)

| Key            | Action                                                  |
|----------------|---------------------------------------------------------|
| `a`            | Add the currently-focused terminal pane to the list. Closes the plugin. |
| `A`            | Add all currently-visible terminal panes. **Stays open** so you can immediately reorder/jump. |
| `d`            | Delete the highlighted pane.                            |
| `j` / `k`      | Navigate down / up. **Wraps** (`j` from bottom → top).  |
| `K` / `J`      | Reorder up / down (saturates at boundaries; no wrap). Persists immediately. |
| `1` … `9`      | Jump to slot 1-9 if it's a live pane (placeholder slots no-op). |
| `Enter` / `l`  | Focus the highlighted pane and close.                   |
| `c` / `Esc`    | Close. **`Ctrl+c` also closes** (preserves today's accidental muscle memory). |
| `/`            | Enter filter mode.                                      |
| `#`            | Enter jump mode.                                        |

### Filter mode (`[F]`)

| Key                   | Action                                                  |
|-----------------------|---------------------------------------------------------|
| any printable char    | Append to the query. Snaps highlight to top match.      |
| `Backspace`           | Remove last character. When the query empties, re-anchor highlight to the focused pane. |
| `↑` / `↓`             | Navigate within the filtered view (saturates).          |
| `Enter`               | Focus the highlighted match and close.                  |
| `Esc` (non-empty query) | Clear the query, stay in filter.                       |
| `Esc` (empty query)   | Drop to command mode.                                   |
| `Ctrl+W` / `Alt+x` / etc. | No-op. Filter mode rejects modified printables to avoid swallowing terminal expectations. |

Slot prefixes (`1  `, `a  `) are **suppressed** in filter mode because rows
are reordered by match score.

### Jump mode (`[J]`)

Read-only. Pressing anything other than a slot key or `Esc` is a no-op.

| Key            | Action                                                  |
|----------------|---------------------------------------------------------|
| `1` … `9`      | Jump to slot 0-8 (matching pane, if live).              |
| `a` … `z`      | Jump to slot 9-34 (letters extend addressability).      |
| `Esc`          | Return to command mode.                                 |

## Slot scheme

35 slots, indexed by pane Vec position:

- `panes[0]` → slot `1`
- `panes[8]` → slot `9`
- `panes[9]` → slot `a`
- `panes[34]` → slot `z`
- `panes[35]+` → no slot shortcut (reachable via filter or arrow nav)

In command mode, only digit slots `1-9` fire jumps (letter keys retain their
command bindings — `a` add, `c` close, `d` delete, etc.). To use letter slots
enter jump mode via `#`.

## Reordering

`K` and `J` in command mode reorder the highlighted pane up or down. The
manual order is canonical and survives session reload — the persistence
schema records each bookmark's saved Vec position.

```
before:        after `K` (selected=2 moves up):
1  alpha       1  alpha
2  beta        2 >gamma   ← gamma now slot 2
3 >gamma  →    3  beta
4  delta       4  delta
```

Pre-conditions: both the selected pane and its neighbor must be live (not
placeholder). Saturating: K at slot 1 is a no-op; J at the last slot is a
no-op.

### Duplicate-title limitation

Two panes with identical `(tab_name, pane_title)` cannot be distinguished
across reload. In-session reorder still persists; restored relative order is
implementation-defined. (Solving this fully requires a stable per-pane
discriminator that survives reload, which the zellij API doesn't expose.)

## Placeholder slots during partial restore

When the persistence file has bookmarks that haven't yet appeared as live
panes (typically a sub-second window after session reopen), those slots
render as `<slot>  ?  (resolving)` rather than collapsing the gap. Pressing
the slot key while it's a placeholder is a no-op — guarantees pressing `2`
always jumps to the pane the user pinned at slot 2 OR no-ops, never to the
wrong pane.

The first time the user mutates the list (`a`/`A`/`d`/`K`/`J`), unresolved
saved-position bookmarks are converted to append-on-resolve and the
in-memory representation compacts to dense.

## Configuration

Bind the invocation key to the `toggle` pipe (`MessagePlugin`), with config
keys inline:

```kdl
shared_except "locked" {
    bind "Ctrl y" {
        MessagePlugin "file:~/.config/zellij/plugins/harpoon.wasm" {
            name "toggle"
            floating true            // REQUIRED: a cold spawn without it lands TILED
            default_mode "command"   // "command" | "filter" | "jump"
            matcher "fuzzy"          // "fuzzy" | "substring"
            show_slots "true"        // "true" | "false" | "yes" | "no" | ...
        }
    }
}
```

All keys except `name`/`floating` are optional. Defaults: `default_mode =
"command"`, `matcher = "fuzzy"`, `show_slots = "true"`. Values are
case-insensitive; unknown values fall back to the default.

**Do NOT bind `LaunchOrFocusPlugin` for invocation.** Its host-side focus
path (`focus_plugin_pane`, zellij ≤ 0.44.3 and current upstream `main`)
carries a double defect — a tab-id/position confusion in `go_to_tab` plus a
silent no-op moving suppressed panes — that jumps the view to an unrelated
tab in tab-churned sessions and strands the menu on its old tab. The
`toggle` pipe never executes that path: harpoon shows/hides itself via
position-correct host calls (see `openspec/specs/pane-pipe-api/spec.md`).

## Persistence

Bookmarks live at `${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon/<session_name>.json`,
one file per zellij session.

### Schema v1 → v2 migration

The v2 schema is a top-level envelope:

```json
{
  "version": 2,
  "bookmarks": [
    { "tab_name": "work", "pane_title": "nvim main.rs", "index": 0 },
    { "tab_name": "shell", "pane_title": "edit log",     "index": 1 }
  ]
}
```

v1 files (bare `Vec<PaneBookmark>` array, no `index` field) are read
transparently with indices assigned in array order. The next save writes v2
form.

**Rollback** (v2 → v1): the v1 binary fails to read the v2 envelope and
starts with empty bookmarks. The v2 file is left untouched. Recommended:
back up the persistence directory before installing v1
(`cp -r ~/.local/share/zellij-harpoon{,.v2-backup}`).

## CLI pipe API

External processes can drive harpoon over `zellij pipe`:

- `jump_pane` — focus a terminal pane by id and leave it fullscreen. Accepts
  `terminal_N` (the `$ZELLIJ_PANE_ID` form) or bare `N`. Correct in plain and
  stacked fullscreen layouts, same-tab and cross-tab, warm or cold plugin
  instance.
- `slot_for_pane` — reverse lookup: prints the 1-based slot currently holding
  a pane id.
- `toggle` — show/hide the menu (the keybind's pipe; see
  [Configuration](#configuration)). Source-agnostic: works from a keybind
  `MessagePlugin`, a CLI `zellij pipe`, or plugin-to-plugin. Shows on the
  invoking tab — warm/instant when the hidden pane is already parked there,
  else by RESPAWNING a fresh instance there (~0.1s) and closing the old one
  (pane relocation is forbidden: zellij's only pane-to-tab mover destroys
  the pane under tab-id/position drift). Hides when already open and
  focused. All decisions run on synchronous host queries, never cached
  event state.

Always target the pipe at the plugin explicitly:

```sh
zellij pipe --name jump_pane \
  --plugin "file:$HOME/.config/zellij/plugins/harpoon.wasm" \
  -- "$ZELLIJ_PANE_ID"
```

Zellij treats the same plugin with a different configuration as a different
pipe destination. The keybind `MessagePlugin { ... }` config participates in
that identity: give CLI pipes a matching `--plugin-configuration
"default_mode=command,matcher=fuzzy,show_slots=true"` (mirroring the keybind
block) to reach the same warm instance instead of spawning a configless twin
(the twin works too — it persists and is reused after its first pipe —
matching just saves the one-time load).

**Never use a broadcast pipe** (omitting `--plugin`) for `jump_pane`: zellij
delivers broadcasts to every running plugin instance, and two harpoon
instances would each run fullscreen normalization — the toggles can cancel
each other.

## Why?

In a sentence: quickly access your most-used panes, type-narrow when the
list grows, jump by slot when you've memorized positions.

- Manually manage a list of favorite panes
- Type-to-filter when the list is long
- Slot-jump (1-9, a-z) for muscle memory
- K/J reorder; saved positions survive reload
- Panes auto-removed from the list when they're closed (post-freeze)

## Installation

**Requires Zellij `0.44.3` or newer** (the plugin uses the synchronous state
queries introduced there — `get_focused_pane_info` / `get_tab_info` — for
ground-truth fullscreen normalization on jumps).

You'll need `wasm32-wasip1` as a Rust target:

```sh
rustup target add wasm32-wasip1
```

Build and install:

```sh
git clone git@github.com:cartwmic/harpoon.git
cd harpoon
cargo build -p harpoon --target wasm32-wasip1 --release
mkdir -p ~/.config/zellij/plugins/
cp target/wasm32-wasip1/release/harpoon.wasm ~/.config/zellij/plugins/
```

Or from the plugin crate directly:

```sh
cd harpoon-plugin && cargo build --release
```

(Cargo doesn't walk per-crate `.cargo/config.toml` from workspace root, so
you need either `cd harpoon-plugin/` OR explicit `--target wasm32-wasip1`
when invoking from root.)

### Permissions & runtime activation

The plugin requests `RunCommands`, `ReadApplicationState`,
`ChangeApplicationState`, `ReadCliPipes` (gates `unblock_cli_pipe_input` /
`cli_pipe_output` — without it every CLI pipe client hangs as a zombie and
`slot_for_pane` produces no output), `OpenTerminalsOrPlugins` (gates
`open_plugin_pane_floating` — the toggle's cross-tab respawn), and
`MessageAndLaunchOtherPlugins` (gates the destination-id `bootstrap_store`
message that hands the live bookmark list to a respawned successor). A denied
response-decoding spawn call PANICS the plugin, so host queries/spawns remain
grant-gated; a hand-off unavailable because its permission was denied degrades
to the successor's existing disk-load fallback.

After deploying a wasm whose permission set grew (e.g. the `ReadCliPipes`
addition), the grant must be renewed at runtime:

1. Deploy: `cp target/wasm32-wasip1/release/harpoon.wasm ~/.config/zellij/plugins/`
2. Open harpoon in a **visible** plugin pane and answer the new permission
   prompt (`y`). A hidden/background instance never shows the prompt, so
   **skipping the visible-pane regrant leaves the new permission inert**.
3. Verify the grant landed: the zellij cache `permissions.kdl` (path from
   `zellij setup --check`, `[CACHE DIR]`) lists `ReadCliPipes`,
   `OpenTerminalsOrPlugins`, and `MessageAndLaunchOtherPlugins` under this
   plugin's path.
4. Restart any long-lived zellij server that predates the grant (e.g. a
   `workspace` session) to clear accumulated wedged pipe clients:
   `zellij kill-session <name>` then reattach.

### Activating the toggle-pipe keybind (pipe-toggle-invocation)

Switching invocation from `LaunchOrFocusPlugin` to the `toggle` pipe is a
config + deploy change (operational — outside this repo's test gate):

1. Deploy the wasm:
   `cp target/wasm32-wasip1/release/harpoon.wasm ~/.config/zellij/plugins/`
2. In the (chezmoi-managed) `~/.config/zellij/config.kdl`, replace the
   `Ctrl y` → `LaunchOrFocusPlugin "file:...harpoon.wasm" { ... }` block
   with the `MessagePlugin` block from [Configuration](#configuration) —
   same plugin URL, `name "toggle"`, and **`floating true`** (a cold spawn
   without it lands as a tiled split). Keep the harpoon config keys
   (`default_mode`/`matcher`/`show_slots`) in the new block so the warm
   instance identity (and any ntfy `--plugin-configuration`) still matches.
3. Reload the config (zellij picks up config.kdl changes live) or restart
   the server session.
4. Answer the `OpenTerminalsOrPlugins` AND
   `MessageAndLaunchOtherPlugins` permission prompt in a VISIBLE plugin pane
   (new grants — same regrant discipline as `ReadCliPipes` above; an
   unanswered prompt leaves toggles inert, while an unavailable bootstrap
   permission degrades cross-tab state transfer to disk load).
5. Verify a round-trip: `Ctrl y` shows the menu floating on the current
   tab → `Esc` hides → switch tab → `Ctrl y` again shows it on the NEW tab
   (menu and view together — the wrong-tab jump is gone; cross-tab invokes
   respawn fresh in ~0.1s, same-tab re-invokes are instant).
6. Until the keybind is swapped, invocation still routes through zellij's
   broken `focus_plugin_pane` path — deploying the wasm alone changes
   nothing about the wrong-tab jump.

## Keybinding

See [Configuration](#configuration) above.

## Custom modifications (this fork)

This fork diverges from `Nacho114/harpoon`:

1. **Three modes** (command/filter/jump) with vim-style Esc transitions.
2. **Type-to-filter** in filter mode with fuzzy (default) or substring matching.
3. **Slot keys** `1-9` (digits) + `a-z` (letters; jump mode only) for fast addressing.
4. **K/J reorder** with persistence — saved position survives session reload.
5. **Sparse `Vec<Option<Pane>>`** internally with placeholder slots during partial restore.
6. **Workspace split** into `harpoon-core` (lib, native+wasm, fully unit-testable, no `zellij-tile` dep) + `harpoon-plugin` (cdylib `harpoon`).
7. **Persistence v2 envelope** with `index: Option<u16>` for saved positions.
8. **`A` does NOT close** — stays open so you can immediately K/J/jump.
9. **`j`/`k` wrap; `K`/`J` saturate** — intentional asymmetry (nav cheap, reorder destructive).
10. **`Ctrl+c` carve-out** — accepts any modifier set, preserves today's accidental close behavior.

Architectural decisions and the full design history live in
[`openspec/changes/add-filter-and-jump-modes/`](./openspec/changes/add-filter-and-jump-modes/)
(13 rounds of blind adversarial review with Claude Opus + GPT-5.4).

## Known issues

- (resolved on the `0.44.3` floor) macOS: `hide_self()` followed by
  `focus_terminal_pane(id)` used to mis-focus a terminal pane instead of
  re-showing the hidden plugin; harpoon temporarily worked around it with
  `close_self()` (commit d6a2039). The quirk does not reproduce on zellij
  0.44.3 (verified 2026-07-09: 10/10 hide/relaunch cycles under the trigger
  condition), so the plugin is persistent again — `scripts/fullscreen-regression.sh`
  scenario S4 guards against regression.

## Contributing

If you find any issues or want to suggest ideas please [open an issue](https://github.com/cartwmic/harpoon/issues/new).

### Development

Make sure you have [rust](https://rustup.rs/) installed then run:

```sh
zellij action new-tab --layout ./plugin-dev-workspace.kdl
```

To run native tests on `harpoon-core`:

```sh
cargo test -p harpoon-core
```

(Currently 217 unit tests covering every dispatch handler, freeze
algorithm, restore resolution, render-layout helpers, matcher backends,
and slot mapping. The plugin shim is a thin FFI wrapper around the
fully-tested core.)
