#!/usr/bin/env bash
# toggle-pipe-probe.sh — R2/R3 risk probes for the pipe-toggle-invocation
# change (tasks 1.1 + 1.2). EVIDENCE GATHERING, not a shipped regression
# scenario (that is scripts/toggle-pipe-regression.sh, task 4.1).
#
#   R2  Does a keybind-source `MessagePlugin` pipe reach the loaded plugin,
#       with what PipeSource, and without permission prompts/denials?
#   R3  While hidden via hide_self(), does the plugin's own pane appear in
#       its cached PaneUpdate manifest (and with is_suppressed set)?
#
# Method: copy the repo to a throwaway dir, insert a TOGGLE_PROBE eprintln at
# the top of `fn pipe` (throwaway instrumentation — never shipped; the
# production `fn pipe` still early-returns on non-CLI sources, which is
# exactly why R2 needs probing), build wasm there, drive a scripted zellij
# session (tmux-hosted; precedent: scripts/cli-pipe-permission-regression.sh)
# with an F6 MessagePlugin keybind, and read the evidence from the zellij
# server log.
#
# Requirements: tmux, zellij >= 0.44.3, cargo + wasm32-wasip1, python3, rsync.
# Safe to re-run; cleans up its session, throwaway dir, and permissions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Fixed cache dir: the 5-minute wasm build survives re-runs (PROBE_FRESH=1 to force).
PROBE_DIR="${TMPDIR:-/tmp}/harpoon-probe-cache"
[ -n "${PROBE_FRESH:-}" ] && rm -rf "$PROBE_DIR"
SES="hprobe$$"
HOST="hprobe-host$$"
PASS=0; FAIL=0

say() { printf '%s\n' "$*"; }
scr() { tmux capture-pane -t "$HOST" -p; }
za()  { zellij -s "$SES" action "$@"; }

wait_for() { # wait_for <label> <grep-pattern> <present|absent>
  local try
  for try in 1 2 3 4 5 6 7 8; do
    if [ "$3" = present ]; then scr | grep -q "$2" && return 0
    else scr | grep -q "$2" || return 0; fi
    sleep 1
  done
  say "WAIT-TIMEOUT $1 (pattern '$2' not $3)"; scr | tail -5; return 1
}

zellij_log() {
  local d
  for d in "${TMPDIR:-/tmp}" /tmp /var/folders/*/*/*; do
    [ -f "$d/zellij-$(id -u)/zellij-log/zellij.log" ] && { echo "$d/zellij-$(id -u)/zellij-log/zellij.log"; return; }
  done
  return 1
}

assert() {
  if [ "$2" -eq 0 ]; then say "PASS $1"; PASS=$((PASS+1)); else say "FAIL $1"; FAIL=$((FAIL+1)); fi
}

cleanup() {
  tmux kill-session -t "$HOST" 2>/dev/null || true
  zellij kill-session "$SES" 2>/dev/null || true
  sleep 1
  zellij delete-session "$SES" --force >/dev/null 2>&1 || true
  # PROBE_DIR (build cache) is intentionally kept; PROBE_FRESH=1 clears it.
  if [ -n "${PERM_CREATED:-}" ]; then rm -f "$PERM_FILE";
  elif [ -n "${PERM_BAK:-}" ] && [ -f "$PERM_BAK" ]; then cp "$PERM_BAK" "$PERM_FILE"; fi
}
trap cleanup EXIT

# ── throwaway copy + probe instrumentation (target/ cache survives re-runs) ─
say "copying repo to throwaway dir $PROBE_DIR ..."
mkdir -p "$PROBE_DIR"
rsync -a --exclude target --exclude .git "$REPO_ROOT/" "$PROBE_DIR/"

python3 - "$PROBE_DIR/harpoon-plugin/src/main.rs" <<'PYEOF'
import sys
path = sys.argv[1]
src = open(path).read()
# Visible-event probe: subscribe + log delivery timing around hide/show.
sub_anchor = "EventType::Key,"
assert src.count(sub_anchor) == 1, "subscribe anchor not unique"
src = src.replace(sub_anchor, sub_anchor + "\n            EventType::Visible,")
vis_anchor = "            Event::TabUpdate(tab_info) => {"
assert src.count(vis_anchor) == 1, "tabupdate anchor not unique"
src = src.replace(vis_anchor, """            Event::Visible(v) => {
                eprintln!(\"VISIBLE_PROBE {v}\");
            }
""" + vis_anchor)

anchor = "fn pipe(&mut self, pipe_message: PipeMessage) -> bool {"
probe = anchor + """
        // TOGGLE_PROBE — throwaway instrumentation (R2/R3 evidence).
        {
            let own_id = get_plugin_ids().plugin_id;
            let own_pane = self.pane_manifest.as_ref().and_then(|m| {
                m.panes.iter().find_map(|(tab_pos, panes)| {
                    panes
                        .iter()
                        .find(|p| p.is_plugin && p.id == own_id)
                        .map(|p| (*tab_pos, p.is_suppressed, p.is_floating))
                })
            });
            let active_tab = self
                .tab_info
                .as_ref()
                .and_then(|ts| ts.iter().find(|t| t.active).map(|t| t.position));
            // Synchronous host queries — freshness check vs the caches above.
            let sync_focused = get_focused_pane_info(); // (stable tab ID, PaneId)
            let sync_focused_tab = sync_focused
                .as_ref()
                .ok()
                .and_then(|(tab_id, _)| get_tab_info(*tab_id))
                .map(|t| (t.position, t.active));
            let sync_own_suppressed = get_pane_info(zellij_tile::prelude::PaneId::Plugin(own_id))
                .map(|p| p.is_suppressed);
            eprintln!(
                "TOGGLE_PROBE source={:?} name={} cached_active_tab={:?} cached_own_pane_tab_suppressed_floating={:?} sync_focused_tab_id={:?} sync_focused_pos_active={:?} sync_own_suppressed={:?}",
                pipe_message.source, pipe_message.name, active_tab, own_pane,
                sync_focused.as_ref().ok().map(|(t, _)| *t), sync_focused_tab, sync_own_suppressed
            );
        }"""
assert src.count(anchor) == 1, "pipe fn anchor not unique"
open(path, "w").write(src.replace(anchor, probe))
print("probe instrumentation inserted")
PYEOF

say "building probe wasm..."
cargo build --release -p harpoon --target wasm32-wasip1 \
  --manifest-path "$PROBE_DIR/Cargo.toml" >/dev/null
WASM="$PROBE_DIR/target/wasm32-wasip1/release/harpoon.wasm"

# ── permission seeding (scriptability; precedent) ──────────────────────────
PERM_FILE="$(zellij setup --check 2>/dev/null | sed -n 's/^\[CACHE DIR\]: //p')/permissions.kdl"
if [ -f "$PERM_FILE" ]; then
  PERM_BAK="$(mktemp)"; cp "$PERM_FILE" "$PERM_BAK"
else
  mkdir -p "$(dirname "$PERM_FILE")"; : > "$PERM_FILE"; PERM_CREATED=1
fi
# Rewrite (never skip) this wasm's entry: a stale block missing a newly
# required permission makes zellij show an interactive prompt the scripted
# session can never answer.
awk -v wasm="$WASM" 'BEGIN{skip=0} $0 == "\"" wasm "\" {" {skip=1; next} skip && /^\}/ {skip=0; next} !skip {print}' "$PERM_FILE" > "$PERM_FILE.tmp" && mv "$PERM_FILE.tmp" "$PERM_FILE"
printf '"%s" {\n    ChangeApplicationState\n    RunCommands\n    ReadApplicationState\n    ReadCliPipes\n    OpenTerminalsOrPlugins\n    MessageAndLaunchOtherPlugins\n}\n' "$WASM" >> "$PERM_FILE"

# ── probe config: F6 → MessagePlugin toggle pipe (merges with defaults) ────
CFG="$PROBE_DIR/probe-config.kdl"
cat > "$CFG" <<EOF
keybinds {
    shared_except "locked" {
        bind "F6" { MessagePlugin "file:$WASM" { name "toggle"; floating true; }; }
    }
}
EOF

LOG="$(zellij_log)" || { say "FATAL zellij log not found"; exit 1; }
LOG_OFFSET="$(wc -l < "$LOG" | tr -cd '0-9')"

probe_lines() { tail -n "+$((LOG_OFFSET + 1))" "$LOG" | grep "TOGGLE_PROBE" || true; }

# ── scripted session ───────────────────────────────────────────────────────
tmux new-session -d -s "$HOST" -x 180 -y 45 "zellij --config $CFG -s $SES"
sleep 4

# Load harpoon visible (floating); verify its pane frame is on screen.
za launch-or-focus-plugin --floating "file:$WASM"; sleep 3
wait_for "harpoon visible after launch" "harpoon" present || { say "FATAL harpoon never appeared"; exit 1; }

# Hide via Esc (hide_self path); verify the frame left the screen. The Esc
# delivery races zellij's focus settling, so retry focus+Esc a few times.
HIDDEN=""
for attempt in 1 2 3 4 5; do
  tmux send-keys -t "$HOST" Escape; sleep 2
  if ! scr | grep -q "harpoon"; then HIDDEN=1; break; fi
  say "NOTE Esc attempt $attempt did not hide; refocusing"
  za launch-or-focus-plugin --floating "file:$WASM"; sleep 3
done
[ -n "$HIDDEN" ] || { say "FATAL cannot reach hidden state"; exit 1; }

# Cross-tab: move to a fresh tab (verify tab bar shows Tab #2), then keybind.
za new-tab; sleep 2
wait_for "second tab active" "Tab #2" present || say "NOTE tab-bar pattern not matched; continuing"
tmux send-keys -t "$HOST" F6; sleep 3

EVIDENCE="$(probe_lines)"
say "---- probe evidence ----"
say "${EVIDENCE:-<none>}"
say "------------------------"

ck() { printf '%s' "$EVIDENCE" | grep -Eq "$1" && echo 0 || echo 1; }

# R2: keybind pipe delivered, with a non-CLI source.
assert "R2a keybind MessagePlugin pipe DELIVERED to loaded plugin" "$(ck 'TOGGLE_PROBE')"
assert "R2b pipe source is NON-CLI (keybind source variant logged above)" "$([ "$(ck 'source=Cli')" -eq 1 ]; echo $?)"
DENIED="$(tail -n "+$((LOG_OFFSET + 1))" "$LOG" | grep -c "denied" || true)"
assert "R2c no permission denials during probe (denied lines: ${DENIED:-0})" "$([ "${DENIED:-0}" -eq 0 ]; echo $?)"

# R3 findings (asserts DOCUMENT the confirmed reality — they fail only if the
# host behavior changes, which would itself be worth knowing):
# R3-cache: cached TabUpdate/PaneUpdate FREEZE while the plugin is suppressed —
#   the cache still claims pre-hide state (tab 0 active, own pane unsuppressed).
assert "R3-cache caches CONFIRMED STALE while suppressed (pre-hide values)" "$(ck 'cached_active_tab=Some\(0\) cached_own_pane_tab_suppressed_floating=Some\(\(0, false, true\)\)')"
# R3-sync: synchronous host queries are FRESH while suppressed —
#   focused tab reports pos 1 active (the new tab), own pane suppressed=true.
assert "R3-sync get_tab_info(focused) is fresh (pos 1, active)" "$(ck 'sync_focused_pos_active=Some\(\(1, true\)\)')"
assert "R3-sync get_pane_info(own) reports suppressed while hidden" "$(ck 'sync_own_suppressed=Some\(true\)')"

# R3-vis: Event::Visible is emitted ONLY to TILED plugin panes
# (zellij-server tab/mod.rs Tab::visible — tiled_panes.pane_ids() filter),
# so the floating harpoon pane NEVER receives it: expect zero probe lines.
VIS="$(tail -n "+$((LOG_OFFSET + 1))" "$LOG" | grep "VISIBLE_PROBE" || true)"
say "visible-event evidence:"; say "${VIS:-<none — confirmed not delivered>}"
[ -z "$VIS" ] && V1=0 || V1=1
assert "R3-vis Event::Visible CONFIRMED NOT DELIVERED to floating plugin pane" "$V1"

say "----"
say "probes: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
