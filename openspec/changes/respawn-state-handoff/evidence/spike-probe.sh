#!/usr/bin/env bash
# Targeted bootstrap hand-off probe. Throwaway. Scratch session only.
set -euo pipefail

WASM="/tmp/spike-handoff/target/wasm32-wasip1/release/spike-handoff.wasm"
SES="spikeho$$"
HOST="spikeho-host$$"
PERMS="$HOME/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl"

say() { printf '%s\n' "$*"; }

zellij_log() {
  local d
  for d in /var/folders/*/*/T; do
    [ -f "$d/zellij-$(id -u)/zellij-log/zellij.log" ] && { echo "$d/zellij-$(id -u)/zellij-log"; return; }
  done
  return 1
}

# --- seed permissions (surgical: add our entry; remove exactly it on cleanup)
python3 - "$PERMS" "$WASM" <<'EOF'
import sys
perms_path, wasm = sys.argv[1], sys.argv[2]
entry = '"%s" {\n    ReadApplicationState\n    ChangeApplicationState\n    OpenTerminalsOrPlugins\n    MessageAndLaunchOtherPlugins\n    ReadCliPipes\n}\n' % wasm
s = open(perms_path).read()
if wasm in s:
    # rewrite (never skip) stale entry: drop old block, append fresh
    import re
    s = re.sub(r'"%s" \{[^}]*\}\n?' % re.escape(wasm), '', s)
open(perms_path, 'w').write(s + entry)
print("seeded")
EOF

cleanup() {
  tmux kill-session -t "$HOST" 2>/dev/null || true
  zellij kill-session "$SES" 2>/dev/null || true
  python3 - "$PERMS" "$WASM" <<'EOF'
import sys, re
perms_path, wasm = sys.argv[1], sys.argv[2]
s = open(perms_path).read()
s = re.sub(r'"%s" \{[^}]*\}\n?' % re.escape(wasm), '', s)
open(perms_path, 'w').write(s)
print("perms entry removed")
EOF
}
trap cleanup EXIT

LOGDIR="$(zellij_log)" || { say "no zellij log dir"; exit 1; }
LOG="$LOGDIR/zellij.log"
MARK_LINES=$(wc -l < "$LOG" 2>/dev/null || echo 0)

# --- scratch session in tmux
tmux new-session -d -s "$HOST" -x 200 -y 50 "zellij -s $SES"
sleep 3

# launch plugin cold via noop pipe (grant lands from seed), then go
timeout 15 zellij -s "$SES" pipe --plugin "file:$WASM" --name noop || say "noop pipe rc=$?"
sleep 2
timeout 15 zellij -s "$SES" pipe --plugin "file:$WASM" --name go -- "file:$WASM" || say "go pipe rc=$?"
sleep 4

say "=== SPIKEHO evidence (new lines only) ==="
# log may have rotated; check both
for f in "$LOG" "$LOGDIR/zellij.log.old.0"; do
  [ -f "$f" ] || continue
  grep -h "SPIKEHO" "$f" 2>/dev/null || true
done | tail -40
say "=== done ==="
