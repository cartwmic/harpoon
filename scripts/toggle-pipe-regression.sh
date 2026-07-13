#!/usr/bin/env bash
# toggle-pipe-regression.sh — regression scenario for the
# pipe-toggle-invocation change (task 4.1).
#
# AC: pane-pipe-api.toggle-pipe-invocation
# AC: pane-pipe-api.toggle-state-sync-query-verified
#
# Drives a REAL zellij session (tmux-hosted; precedent:
# scripts/cli-pipe-permission-regression.sh) against the freshly built
# harpoon.wasm with a `MessagePlugin`-style `toggle` keybind (F6) and asserts:
#   S1  cold spawn: first F6 shows the menu on the invoking tab (no cached
#       event state exists at that point — pipe precedes first TabUpdate)
#   S2  visible → toggle hides (menu leaves every tab's floating set)
#   S3  same-tab re-invoke after Esc-close shows menu+view on the invoking tab
#   S4  cross-tab invoke AFTER a tab close (forcing tab-id/position drift)
#       lands menu AND view on the invoking tab — the double-defect killer:
#       zellij's focus_plugin_pane would jump the view to a stale-id tab and
#       strand the menu on its home tab; the toggle pipe never calls it
#
# Ground truth comes from `zellij action dump-layout`: the focused tab carries
# focus=true and a visible harpoon menu appears as a floating plugin pane in
# exactly one tab's floating_panes block. Suppressed (hidden) panes are not
# part of the serialized layout.
#
# Requirements: tmux, zellij >= 0.44.3, cargo + wasm32-wasip1 target.
# Safe to re-run; cleans up its session and restores plugin permissions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM="$REPO_ROOT/target/wasm32-wasip1/release/harpoon.wasm"
SES="htoggle$$"
HOST="htoggle-host$$"
PASS=0; FAIL=0

say() { printf '%s\n' "$*"; }
scr() { tmux capture-pane -t "$HOST" -p; }
za()  { zellij -s "$SES" action "$@"; }

assert() {
  if [ "$2" -eq 0 ]; then say "PASS $1"; PASS=$((PASS+1)); else say "FAIL $1"; FAIL=$((FAIL+1)); fi
}

# Parse focused tab + harpoon-holding tab from ONE captured dump-layout.
# (Never pipe the zellij client into an early-closing reader like `head` —
# the client panics on EPIPE when the reader exits first.)
focused_tab_of() { printf '%s\n' "$1" | sed -nE 's/.*tab name="([^"]+)".*focus=true.*/\1/p' | head -1; }
harpoon_tab_of() {
  printf '%s\n' "$1" | awk '
    /^[[:space:]]*tab name="/ {
      match($0, /name="[^"]+"/); tab = substr($0, RSTART+6, RLENGTH-7)
    }
    /harpoon\.wasm/ { if (tab != "") { print tab; exit } }
  '
}

# Wait until focused==$2 and harpoon-tab==$3 ("" = hidden); retry loop
# absorbs zellij's asynchronous layout settling.
expect_state() { # expect_state <label> <focused> <harpoon-tab-or-empty>
  local label="$1" want_focus="$2" want_harpoon="$3" L f h try
  for try in 1 2 3 4 5 6; do
    L="$(za dump-layout 2>/dev/null || true)"
    f="$(focused_tab_of "$L")"; h="$(harpoon_tab_of "$L")"
    [ "$f" = "$want_focus" ] && [ "$h" = "$want_harpoon" ] && { assert "$label (focus=$f harpoon=${h:-hidden})" 0; return; }
    sleep 1
  done
  say "  state: focused_tab='$f' harpoon_tab='${h:-<hidden>}' wanted focus='$want_focus' harpoon='${want_harpoon:-<hidden>}'"
  assert "$label" 1
}

press() { tmux send-keys -t "$HOST" "$1"; sleep 2; }

cleanup() {
  tmux kill-session -t "$HOST" 2>/dev/null || true
  zellij kill-session "$SES" 2>/dev/null || true
  sleep 1
  zellij delete-session "$SES" --force >/dev/null 2>&1 || true
  if [ -n "${PERM_CREATED:-}" ]; then rm -f "$PERM_FILE";
  elif [ -n "${PERM_BAK:-}" ] && [ -f "$PERM_BAK" ]; then cp "$PERM_BAK" "$PERM_FILE"; fi
  rm -f "$CFG"
}
trap cleanup EXIT

# ── build + permission seeding ─────────────────────────────────────────────
say "building wasm..."
cargo build --release -p harpoon --target wasm32-wasip1 \
  --manifest-path "$REPO_ROOT/Cargo.toml" >/dev/null

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

# ── production-shaped keybind: MessagePlugin toggle pipe on F6 ─────────────
# BSD mktemp requires trailing Xs (a .kdl suffix template silently creates a
# literal file and collides on re-run); zellij accepts any extension.
CFG="$(mktemp "${TMPDIR:-/tmp}/harpoon-toggle-cfg.XXXXXX")"
cat > "$CFG" <<EOF
keybinds {
    shared_except "locked" {
        bind "F6" { MessagePlugin "file:$WASM" { name "toggle"; floating true; }; }
    }
}
EOF

# ── session topology: three named tabs; close the middle one for id drift ──
tmux new-session -d -s "$HOST" -x 180 -y 45 "zellij --config $CFG -s $SES"
for try in 1 2 3 4 5 6 7 8 9 10; do
  zellij list-sessions 2>/dev/null | grep -q "$SES" && break
  sleep 2
done
zellij list-sessions 2>/dev/null | grep -q "$SES" || { say "FATAL zellij session never came up"; exit 1; }
sleep 2
za rename-tab T1; sleep 1
za new-tab; sleep 1; za rename-tab T2; sleep 1
za new-tab; sleep 1; za rename-tab T3; sleep 1

# ── S1: cold spawn shows on the invoking tab (T3) ──────────────────────────
press F6
expect_state "S1 cold-spawn toggle shows menu+view on invoking tab T3" T3 T3

# ── S2: visible → toggle hides ─────────────────────────────────────────────
press F6
expect_state "S2 visible toggle hides the menu (view stays on T3)" T3 ""

# ── S3: same-tab re-invoke after Esc-close ─────────────────────────────────
press F6
expect_state "S3-pre menu shown again on T3" T3 T3
# Esc-close (mode-state-machine Close consolidation path), with retry —
# Esc delivery races zellij focus settling (see toggle-pipe-probe.sh).
HIDDEN=""
for attempt in 1 2 3 4 5; do
  press Escape
  [ -z "$(harpoon_tab_of "$(za dump-layout 2>/dev/null || true)")" ] && { HIDDEN=1; break; }
  say "NOTE Esc attempt $attempt did not hide; refocusing via F6 x2"
  press F6; press F6
done
[ -n "$HIDDEN" ] || { say "FATAL cannot reach hidden state via Esc"; exit 1; }
press F6
expect_state "S3 same-tab re-invoke after Esc-close lands on T3" T3 T3
press F6   # hide again for S4
expect_state "S3-post hidden again (parked on T3)" T3 ""

# ── S4: cross-tab invoke after tab close (id/position drift) ───────────────
# Close T2: T3's stable id now exceeds its position — the exact drift that
# made zellij's focus_plugin_pane jump to an unrelated tab ("Amazon" bug).
za go-to-tab-name T2; sleep 1
za close-tab; sleep 2
za go-to-tab-name T1; sleep 1
press F6
expect_state "S4 cross-tab invoke under id/position drift lands menu+view on T1" T1 T1

# ── S5: invoke FROM the drifted tab — T3's stable id exceeds its position ──
# The cross-tab branch RESPAWNS a fresh instance on the invoking tab
# (owner-ruled mechanism; pane relocation is forbidden — the break host
# call destroys the pane under id/position drift, upstream defect #3).
# S5 exercises that respawn under drift: menu+view must land on T3 and the
# old pane on T1 must be gone (a lingering T1 pane would match first in the
# layout parse and fail this assertion).
press F6   # hide again (parks on T1)
expect_state "S5-pre hidden again (parked on T1)" T1 ""
za go-to-tab-name T3; sleep 1
press F6
sleep 2   # respawn = fresh wasm load (~100ms) + bookmark restore
expect_state "S5 invoke from drifted tab T3 (id>position) respawns menu+view on T3" T3 T3

# ── S6: the respawned instance is the keybind's pipe destination ──────────
# Identical URL + configuration ⇒ the next keybind toggle must reach the
# fresh instance and hide it (proves single-instance addressing survived
# the respawn).
press F6
expect_state "S6 keybind pipe reaches respawned instance (toggles hide)" T3 ""

# ── S7: jump_pane cold spawn must NOT poison the parked-tab record ────────
# Round-2 review P1: a cold `jump_pane` pipe spawns the plugin on the
# then-active tab while focusing a terminal on ANOTHER tab; a
# focused-tab-proxy parked record taken at grant time would then make a
# later toggle warm-show on the WRONG tab. Fresh session: pane on J1,
# invoke the cold jump FROM J2 (plugin parks on J2, jump focuses J1), then
# toggle from J1 — menu+view must land on J1 (respawn; never a warm show
# on J2).
SES2="htogglej$$"
HOST2="htogglej-host$$"
scr2() { tmux capture-pane -t "$HOST2" -p; }
za2()  { zellij -s "$SES2" action "$@"; }
cleanup2() {
  tmux kill-session -t "$HOST2" 2>/dev/null || true
  zellij kill-session "$SES2" 2>/dev/null || true
  sleep 1
  zellij delete-session "$SES2" --force >/dev/null 2>&1 || true
}
trap 'cleanup; cleanup2' EXIT

tmux new-session -d -s "$HOST2" -x 180 -y 45 "zellij --config $CFG -s $SES2"
for try in 1 2 3 4 5 6 7 8 9 10; do
  zellij list-sessions 2>/dev/null | grep -q "$SES2" && break
  sleep 2
done
sleep 2
# Dismiss the fresh-session "About Zellij" tip pane — it steals focus and
# would swallow the marker keystrokes.
tmux send-keys -t "$HOST2" Escape; sleep 1
za2 rename-tab J1; sleep 1
# Mark the J1 pane to learn its id (precedent: cli-pipe-permission-regression).
J1_ID=""
for try in 1 2 3 4 5; do
  za2 write-chars "clear; echo MARK_J1=\$ZELLIJ_PANE_ID"; sleep 1
  za2 write 13; sleep 1.5
  J1_ID="$(scr2 | sed -nE 's/.*MARK_J1=(terminal_)?([0-9]+).*/\2/p' | head -1)"
  [ -n "$J1_ID" ] && break
  tmux send-keys -t "$HOST2" Escape; sleep 1
done
[ -n "$J1_ID" ] || { say "FATAL could not resolve J1 pane id"; exit 1; }
za2 new-tab; sleep 1; za2 rename-tab J2; sleep 1
# Cold jump_pane FROM J2: plugin spawns parked on J2; jump focuses J1's pane.
timeout 20 zellij -s "$SES2" pipe --name jump_pane --plugin "file:$WASM" -- "terminal_$J1_ID" || true
f2=""
for try in 1 2 3 4 5 6; do
  L2="$(za2 dump-layout 2>/dev/null || true)"
  f2="$(focused_tab_of "$L2")"
  [ "$f2" = J1 ] && break
  sleep 1
done
assert "S7-pre cold jump landed the view on J1 (focus=$f2)" "$([ "$f2" = J1 ]; echo $?)"
# Toggle from J1: menu+view must land HERE, not warm-show on J2.
tmux send-keys -t "$HOST2" F6; sleep 4
L2="$(za2 dump-layout 2>/dev/null || true)"
f2="$(focused_tab_of "$L2")"; h2="$(harpoon_tab_of "$L2")"
assert "S7 toggle after cold jump_pane lands menu+view on J1 (focus=$f2 harpoon=${h2:-hidden})" "$([ "$f2" = J1 ] && [ "$h2" = J1 ]; echo $?)"

# ── S8: cross-tab respawn presents persisted targets (state hand-off) ────
# AC pane-pipe-api.respawn-state-hand-off: bookmark a pane on T3, then
# invoke cross-tab from T1 — the respawned successor's menu must list the
# bookmark (the outgoing instance hands its store over; no
# invoke-again-to-recover). NOTE on timing granularity: tmux capture
# resolution (~seconds) cannot distinguish hand-off (0ms) from a fast disk
# load; the instant-adoption property itself is covered by native core
# tests — this scenario asserts the end-to-end functional outcome.
za go-to-tab-name T3; sleep 1
za rename-pane BMARK1; sleep 1
press F6
expect_state "S8-pre menu shown on T3 for bookmarking" T3 T3
tmux send-keys -t "$HOST" a; sleep 2   # add focused pane (BMARK1) → Effect::Save
press Escape                            # close → parked on T3
expect_state "S8-mid hidden after bookmark add" T3 ""
za go-to-tab-name T1; sleep 1
press F6
sleep 2   # respawn = fresh wasm load + bootstrap adoption
L="$(za dump-layout 2>/dev/null || true)"
SCREEN="$(scr)"
f="$(focused_tab_of "$L")"; h="$(harpoon_tab_of "$L")"
BM_SHOWN=1; echo "$SCREEN" | grep -q "BMARK1" && BM_SHOWN=0
assert "S8 cross-tab respawn menu lists persisted target BMARK1 (focus=$f harpoon=${h:-hidden})" \
  "$([ "$f" = T1 ] && [ "$h" = T1 ] && [ "$BM_SHOWN" -eq 0 ]; echo $?)"
DATA_FILE="${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon/$SES.json"
BM_DISK=1; grep -q "BMARK1" "$DATA_FILE" 2>/dev/null && BM_DISK=0
assert "S8-disk persisted file contains BMARK1" "$BM_DISK"

# ── S9: re-delivered CLI toggle does not hide the fresh menu ────────────
# AC pane-pipe-api.duplicate-toggle-delivery-tolerance: a CLI-sourced
# cross-tab toggle triggers a respawn AND zellij re-delivers the SAME
# still-open CLI pipe to the successor (~380ms, probe 2026-07-13). Without
# the identity guard the re-delivery hides the just-shown menu; with it the
# menu stays AND the CLI client is still released (no hang → no exit 124).
press F6   # hide (parked on T1)
expect_state "S9-pre hidden (parked on T1)" T1 ""
za go-to-tab-name T3; sleep 1
CLI_RC=0
timeout 20 zellij -s "$SES" pipe --name toggle --plugin "file:$WASM" -- "" || CLI_RC=$?
assert "S9-release CLI toggle client released promptly (rc=$CLI_RC)" "$([ "$CLI_RC" -eq 0 ]; echo $?)"
sleep 3   # cover the re-delivery window before asserting
expect_state "S9 menu still shown on T3 after stale re-delivery window" T3 T3

# ── S10: disk file never shrinks across respawn cycles (prune-guard) ─────
# AC reorder.destructive-save-guard: every cross-tab respawn is a fresh
# instance whose early saves (pre-baseline, pre-manifest) must never shrink
# the persisted bookmark set.
count_bm() { python3 -c "import json,sys;d=json.load(open('$DATA_FILE'));print(len(d['bookmarks']))" 2>/dev/null || echo 0; }
BM_BEFORE="$(count_bm)"
press F6   # hide (parked on T3)
za go-to-tab-name T1; sleep 1
press F6; sleep 2   # respawn cycle 1 → menu on T1
press F6            # hide (parked on T1)
za go-to-tab-name T3; sleep 1
press F6; sleep 2   # respawn cycle 2 → menu on T3
BM_AFTER="$(count_bm)"
assert "S10 persisted bookmarks never shrink across respawn cycles (before=$BM_BEFORE after=$BM_AFTER)" \
  "$([ "$BM_AFTER" -ge "$BM_BEFORE" ] && [ "$BM_BEFORE" -ge 1 ]; echo $?)"

# (Permission-denied degrade is NOT scriptable end-to-end: an unseeded
# permission makes zellij raise an interactive prompt the scripted session
# can never answer — the deny path is grant-gated in the shim and covered
# by native core tests + the pre-grant deny-safe adoption design.)

say "----"
say "scenarios: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
