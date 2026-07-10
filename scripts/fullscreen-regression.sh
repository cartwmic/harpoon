#!/usr/bin/env bash
# fullscreen-regression.sh — mandatory regression scenarios for the
# fix-cross-tab-fullscreen-normalization change (intent.md, frozen ad1963c).
#
# Drives a REAL zellij session (hosted inside tmux so it can be scripted and
# screen-scraped) against the freshly built harpoon.wasm and asserts the four
# scenarios:
#   S1  cold-start pipe -> hidden pane of a fullscreen tab      (ends fullscreen)
#   S2  cold-start pipe -> the fullscreened pane itself         (stays fullscreen)
#   S3  warm cross-tab pipe (persistent instance, no new load)  (ends fullscreen)
#   S4  hide -> relaunch cycle under a fullscreen terminal pane (re-shows, focused,
#       zero new plugin loads — the d6a2039 mis-focus-quirk detector)
#
# Requirements: tmux, zellij >= 0.44.3, cargo + wasm32-wasip1 target.
# Safe to re-run; cleans up its session and restores plugin permissions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM="$REPO_ROOT/target/wasm32-wasip1/release/harpoon.wasm"
SES="hreg$$"
HOST="hreg-host$$"
PASS=0; FAIL=0

# ── helpers ────────────────────────────────────────────────────────────────
say()  { printf '%s\n' "$*"; }
scr()  { tmux capture-pane -t "$HOST" -p; }
za()   { zellij -s "$SES" action "$@"; }
pipe_jump() { timeout 20 zellij -s "$SES" pipe --name jump_pane --plugin "file:$WASM" -- "$1"; }

zellij_log() {
  # macOS: $TMPDIR/zellij-<uid>/zellij-log/zellij.log ; linux: /tmp/zellij-<uid>/...
  local d
  for d in "${TMPDIR:-/tmp}" /tmp /var/folders/*/*/*; do
    [ -f "$d/zellij-$(id -u)/zellij-log/zellij.log" ] && { echo "$d/zellij-$(id -u)/zellij-log/zellij.log"; return; }
  done
  return 1
}

loads() { grep -c "Loaded plugin '$WASM'" "$(zellij_log)" 2>/dev/null || echo 0; }

assert() { # assert <label> <condition-result 0|nonzero>
  if [ "$2" -eq 0 ]; then say "PASS $1"; PASS=$((PASS+1)); else say "FAIL $1"; FAIL=$((FAIL+1)); fi
}

fullscreen_showing() { # true when no side-by-side split separators visible
  [ "$(scr | grep -c '││')" -eq 0 ]
}

mark_pane() { # mark_pane <LABEL> — stamp the focused pane; poll until executed
  # $ZELLIJ_PANE_ID is exported as `terminal_N` on some setups and bare `N` on
  # others — accept both (mirrors parse_pane_id).
  local label="$1" try
  for try in 1 2 3 4 5; do
    za write-chars "clear; echo MARK_${label}=\$ZELLIJ_PANE_ID"
    sleep 1
    za write 13
    sleep 1.5
    scr | grep -Eq "MARK_${label}=(terminal_)?[0-9]+" && return 0
    sleep 2
  done
  return 1
}

pane_id_of() { # pane_id_of <LABEL> — numeric pane id from the screen marker
  scr | sed -nE "s/.*MARK_$1=(terminal_)?([0-9]+).*/\2/p" | head -1
}

cleanup() {
  tmux kill-session -t "$HOST" 2>/dev/null || true
  zellij kill-session "$SES" 2>/dev/null || true
  sleep 1
  zellij delete-session "$SES" --force >/dev/null 2>&1 || true
  if [ -n "${PERM_BAK:-}" ] && [ -f "$PERM_BAK" ]; then cp "$PERM_BAK" "$PERM_FILE"; fi
}
trap cleanup EXIT

# ── build + permission seeding ─────────────────────────────────────────────
say "building wasm..."
cargo build --release -p harpoon --target wasm32-wasip1 --manifest-path "$REPO_ROOT/Cargo.toml" >/dev/null

PERM_FILE="$(zellij setup --check 2>/dev/null | sed -n 's/^\[CACHE DIR\]: //p')/permissions.kdl"
if [ -f "$PERM_FILE" ] && ! grep -qF "\"$WASM\"" "$PERM_FILE"; then
  PERM_BAK="$(mktemp)"; cp "$PERM_FILE" "$PERM_BAK"
  printf '"%s" {\n    ChangeApplicationState\n    RunCommands\n    ReadApplicationState\n}\n' "$WASM" >> "$PERM_FILE"
fi

# ── session topology ───────────────────────────────────────────────────────
# tab1: one pane (origin). tab2: two panes A(left)+B(right); B fullscreened.
# NOTE: tab1's initial pane is left untouched — the default-layout shell has
# been observed to discard scripted input; all markers live on tab2, and "we
# left tab1" is proven by tab2's fullscreen marker being visible.
tmux new-session -d -s "$HOST" -x 180 -y 45 "zellij -s $SES"
sleep 4
za new-tab; sleep 2
mark_pane A || { say "FATAL pane A not markable"; exit 1; }
A_ID="$(pane_id_of A)"
za new-pane -d right; sleep 2
mark_pane B || { say "FATAL pane B not markable"; exit 1; }
B_ID="$(pane_id_of B)"
[ -n "$A_ID" ] && [ -n "$B_ID" ] && [ "$A_ID" != "$B_ID" ] || { say "FATAL could not resolve pane ids"; exit 1; }
za toggle-fullscreen; sleep 1   # tab2 fullscreen on B
za go-to-tab 1; sleep 1

# ── S1: cold-start pipe -> hidden pane (A) of fullscreen tab2 ─────────────
L0="$(loads)"
pipe_jump "terminal_$A_ID"; sleep 2
S1_OK=1
if fullscreen_showing && scr | grep -q "MARK_A="; then S1_OK=0; fi
assert "S1 cold-start hidden-target lands fullscreen" "$S1_OK"
L1="$(loads)"
assert "S1 was cold (new plugin load occurred)" "$([ "$L1" -gt "$L0" ]; echo $?)"

# ── S2: pipe -> the fullscreened pane itself (A is fullscreen now) ─────────
za go-to-tab 1; sleep 1
pipe_jump "terminal_$A_ID"; sleep 2
S2_OK=1
if fullscreen_showing && scr | grep -q "MARK_A="; then S2_OK=0; fi
assert "S2 fullscreened-target-itself stays fullscreen" "$S2_OK"

# ── S3: warm cross-tab pipe (persistent instance, zero new loads) ──────────
za go-to-tab 1; sleep 1
L2="$(loads)"
pipe_jump "terminal_$B_ID"; sleep 2
S3_OK=1
if fullscreen_showing && scr | grep -q "MARK_B="; then S3_OK=0; fi
assert "S3 warm cross-tab lands fullscreen" "$S3_OK"
L3="$(loads)"
assert "S3 reused persistent instance (no new load)" "$([ "$L3" -eq "$L2" ]; echo $?)"

# ── S4: hide -> relaunch cycles under fullscreen (quirk detector) ──────────
# B is fullscreen on tab2 (from S3). Launch harpoon UI, Esc-hide, relaunch x5.
LAUNCH=(launch-or-focus-plugin --floating --move-to-focused-tab
        -c 'default_mode=command,matcher=fuzzy,show_slots=true' "file:$WASM")
za "${LAUNCH[@]}"; sleep 2
L4="$(loads)"
S4_OK=0
for i in 1 2 3 4 5; do
  za write 27; sleep 1                       # Esc -> hide (proves plugin had focus)
  if scr | grep -q 'harpoon ─'; then S4_OK=1; break; fi
  za "${LAUNCH[@]}"; sleep 1                 # relaunch -> must re-show
  if ! scr | grep -q 'harpoon ─'; then S4_OK=1; break; fi
done
assert "S4 hide/relaunch x5 re-shows with focus (quirk dead)" "$S4_OK"
L5="$(loads)"
assert "S4 zero new loads across cycles (instance persists)" "$([ "$L5" -eq "$L4" ]; echo $?)"

say "----"
say "scenarios: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
