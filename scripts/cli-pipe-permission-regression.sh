#!/usr/bin/env bash
# cli-pipe-permission-regression.sh — regression scenario for the
# request-read-cli-pipes-permission change.
#
# AC: pane-pipe-api.host-call-permission-completeness
#
# Drives a REAL zellij session (tmux-hosted, precedent:
# scripts/fullscreen-regression.sh) against the freshly built harpoon.wasm
# with ReadCliPipes GRANTED (seeded permissions.kdl) and asserts:
#   S1  CLI jump_pane pipe client exits promptly (rc 0 — no hang, no
#       `timeout` exit 124; the unblock_cli_pipe_input release holds)
#   S2  CLI slot_for_pane pipe client exits promptly (cli_pipe_output path)
#   S3  the zellij server log gains NO "ReadCliPipes' denied" lines across
#       the whole run
#
# NOTE: permissions are SEEDED here for scriptability. Production activation
# still requires answering the permission prompt in a VISIBLE plugin pane
# (see tasks.md 3.1 — operational, outside gate assertions).
#
# Requirements: tmux, zellij >= 0.44.3, cargo + wasm32-wasip1 target.
# Safe to re-run; cleans up its session and restores plugin permissions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM="$REPO_ROOT/target/wasm32-wasip1/release/harpoon.wasm"
SES="hperm$$"
HOST="hperm-host$$"
PASS=0; FAIL=0

# ── helpers (precedent: fullscreen-regression.sh) ──────────────────────────
say()  { printf '%s\n' "$*"; }
scr()  { tmux capture-pane -t "$HOST" -p; }
za()   { zellij -s "$SES" action "$@"; }

zellij_log() {
  local d
  for d in "${TMPDIR:-/tmp}" /tmp /var/folders/*/*/*; do
    [ -f "$d/zellij-$(id -u)/zellij-log/zellij.log" ] && { echo "$d/zellij-$(id -u)/zellij-log/zellij.log"; return; }
  done
  return 1
}

denied_count() { # occurrences of the ReadCliPipes permission denial in the server log
  local n
  n="$(grep -c "ReadCliPipes' denied" "$(zellij_log)" 2>/dev/null || true)"
  n="$(printf '%s' "$n" | head -1 | tr -cd '0-9')"
  echo "${n:-0}"
}

assert() { # assert <label> <condition-result 0|nonzero>
  if [ "$2" -eq 0 ]; then say "PASS $1"; PASS=$((PASS+1)); else say "FAIL $1"; FAIL=$((FAIL+1)); fi
}

mark_pane() { # mark_pane <LABEL> — stamp the focused pane; poll until executed
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
  if [ -n "${PERM_CREATED:-}" ]; then rm -f "$PERM_FILE";
  elif [ -n "${PERM_BAK:-}" ] && [ -f "$PERM_BAK" ]; then cp "$PERM_BAK" "$PERM_FILE"; fi
}
trap cleanup EXIT

# ── build + permission seeding (ReadCliPipes INCLUDED — the granted state) ─
say "building wasm..."
cargo build --release -p harpoon --target wasm32-wasip1 --manifest-path "$REPO_ROOT/Cargo.toml" >/dev/null

PERM_FILE="$(zellij setup --check 2>/dev/null | sed -n 's/^\[CACHE DIR\]: //p')/permissions.kdl"
if [ -f "$PERM_FILE" ]; then
  PERM_BAK="$(mktemp)"; cp "$PERM_FILE" "$PERM_BAK"
else
  mkdir -p "$(dirname "$PERM_FILE")"
  : > "$PERM_FILE"
  PERM_CREATED=1
fi
grep -qF "\"$WASM\"" "$PERM_FILE" || printf '"%s" {\n    ChangeApplicationState\n    RunCommands\n    ReadApplicationState\n    ReadCliPipes\n}\n' "$WASM" >> "$PERM_FILE"

# ── session topology: one tab, two panes; jump target = pane A ─────────────
tmux new-session -d -s "$HOST" -x 180 -y 45 "zellij -s $SES"
sleep 4
za new-tab; sleep 2
mark_pane A || { say "FATAL pane A not markable"; exit 1; }
A_ID="$(pane_id_of A)"
[ -n "$A_ID" ] || { say "FATAL could not resolve pane id"; exit 1; }
za new-pane -d right; sleep 2

D0="$(denied_count)"

# ── S1: jump_pane pipe client released promptly ────────────────────────────
timeout 20 zellij -s "$SES" pipe --name jump_pane --plugin "file:$WASM" -- "terminal_$A_ID" \
  && RC1=0 || RC1=$?
sleep 2
assert "S1 jump_pane pipe client exits promptly (rc=$RC1, no timeout-124 hang)" "$RC1"

# ── S2: slot_for_pane pipe client released promptly (cli_pipe_output) ──────
timeout 20 zellij -s "$SES" pipe --name slot_for_pane --plugin "file:$WASM" -- "terminal_$A_ID" \
  && RC2=0 || RC2=$?
sleep 2
assert "S2 slot_for_pane pipe client exits promptly (rc=$RC2)" "$RC2"

# ── S3: no ReadCliPipes denials appeared during the run ────────────────────
D1="$(denied_count)"
assert "S3 server log gained no ReadCliPipes-denied lines ($D0 -> $D1)" "$([ "$D1" -eq "$D0" ]; echo $?)"

say "----"
say "scenarios: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
