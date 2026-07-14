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

# Deterministic instrumentation for host states the zellij public API cannot
# force from a scenario client:
# - first ACTUAL render is suppressed until bootstrap/disk readiness;
# - a manifest is not "full" until it covers every known tab position.
# These are core decisions, not shim guesses (Constitution I).
CORE_TEST_RC=0
cargo test -p harpoon-core --manifest-path "$REPO_ROOT/Cargo.toml" \
  first_render_waits_for_full_store_projection_or_terminal_denial >/dev/null 2>&1 || CORE_TEST_RC=$?
assert "S0 first-render full-store projection instrumentation" "$CORE_TEST_RC"
CORE_TEST_RC=0
cargo test -p harpoon-core --manifest-path "$REPO_ROOT/Cargo.toml" \
  full_manifest_requires_coverage_of_every_known_tab >/dev/null 2>&1 || CORE_TEST_RC=$?
assert "S0 partial-manifest readiness instrumentation" "$CORE_TEST_RC"
CORE_TEST_RC=0
cargo test -p harpoon-core --manifest-path "$REPO_ROOT/Cargo.toml" \
  deferred_prune_resumes_once_ready_and_compacts_slots >/dev/null 2>&1 || CORE_TEST_RC=$?
assert "S0 deferred prune resumes after readiness instrumentation" "$CORE_TEST_RC"
CORE_TEST_RC=0
cargo test -p harpoon-core --manifest-path "$REPO_ROOT/Cargo.toml" \
  disk_ids_are_cleared_before_cross_restart_identity_use >/dev/null 2>&1 || CORE_TEST_RC=$?
assert "S0 stale pane-id collision instrumentation" "$CORE_TEST_RC"
CORE_TEST_RC=0
cargo test -p harpoon-core --manifest-path "$REPO_ROOT/Cargo.toml" \
  deterministic_respawn_race_sequence_projects_all_rows_and_guards_disk >/dev/null 2>&1 || CORE_TEST_RC=$?
assert "S0 deterministic bootstrap→partial-manifest→ready-prune sequence" "$CORE_TEST_RC"

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
# invoke cross-tab from T1 WITH THE DISK FILE REMOVED. The first observed
# menu can contain BMARK1 only via targeted bootstrap (disk fallback is
# empty), making this distinguish hand-off from eventual disk recovery.
za go-to-tab-name T3; sleep 1
za rename-pane BMARK1; sleep 1
press F6
expect_state "S8-pre menu shown on T3 for bookmarking" T3 T3
tmux send-keys -t "$HOST" a; sleep 2   # add focused pane (BMARK1) → Effect::Save
DATA_FILE="${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon/$SES.json"
grep -q "BMARK1" "$DATA_FILE" 2>/dev/null || { say "FATAL S8 source bookmark did not persist"; exit 1; }
press Escape                            # close → parked on T3
expect_state "S8-mid hidden after bookmark add" T3 ""
rm -f "$DATA_FILE"                     # force disk fallback to empty
za go-to-tab-name T1; sleep 1
# Do NOT use press(): it sleeps 2s after send and would miss the first frame.
tmux send-keys -t "$HOST" F6
FIRST_MENU_RC=1
SCREEN=""
for try in $(seq 1 150); do
  SCREEN="$(scr)"
  if echo "$SCREEN" | grep -q "===="; then
    echo "$SCREEN" | grep -q "BMARK1" && FIRST_MENU_RC=0
    break
  fi
  sleep 0.02
done
L="$(za dump-layout 2>/dev/null || true)"
f="$(focused_tab_of "$L")"; h="$(harpoon_tab_of "$L")"
assert "S8 first observed cross-tab menu gets BMARK1 from hand-off with disk absent (focus=$f harpoon=${h:-hidden})" \
  "$([ "$f" = T1 ] && [ "$h" = T1 ] && [ "$FIRST_MENU_RC" -eq 0 ]; echo $?)"
BM_DISK=1
for try in $(seq 1 100); do
  grep -q "BMARK1" "$DATA_FILE" 2>/dev/null && { BM_DISK=0; break; }
  sleep 0.02
done
assert "S8 late disk reconcile re-persists handed-off BMARK1" "$BM_DISK"

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
SHRINK_MARKER="$(mktemp)"; SHRINK_STOP="$(mktemp)"; rm -f "$SHRINK_STOP"
python3 - "$DATA_FILE" "$BM_BEFORE" "$SHRINK_MARKER" "$SHRINK_STOP" <<'PY' &
import json, pathlib, sys, time
path, baseline, marker, stop = pathlib.Path(sys.argv[1]), int(sys.argv[2]), pathlib.Path(sys.argv[3]), pathlib.Path(sys.argv[4])
while not stop.exists():
    try:
        data = json.loads(path.read_text())
        if len(data["bookmarks"]) < baseline:
            marker.write_text(str(len(data["bookmarks"])))
            break
    except (OSError, ValueError, KeyError, TypeError):
        pass  # ignore in-flight/nonexistent file; only valid JSON can prove shrink
    time.sleep(0.005)
PY
MONITOR_PID=$!
press F6   # hide (parked on T3)
za go-to-tab-name T1; sleep 1
press F6; sleep 2   # respawn cycle 1 → menu on T1
press F6            # hide (parked on T1)
za go-to-tab-name T3; sleep 1
press F6; sleep 2   # respawn cycle 2 → menu on T3
touch "$SHRINK_STOP"; wait "$MONITOR_PID" || true
BM_AFTER="$(count_bm)"
SHRINK_RC=0; [ -s "$SHRINK_MARKER" ] && SHRINK_RC=1
assert "S10 continuous monitor observes no valid shrinking disk state (before=$BM_BEFORE after=$BM_AFTER)" \
  "$([ "$SHRINK_RC" -eq 0 ] && [ "$BM_AFTER" -ge "$BM_BEFORE" ] && [ "$BM_BEFORE" -ge 1 ]; echo $?)"
rm -f "$SHRINK_MARKER" "$SHRINK_STOP"

# ── S11: aggregate permission denial is deny-safe (scope expansion #1) ──
# Zellij 0.44.3 returns ONE PermissionRequestResult for the whole vector and
# blocks plugin events while the prompt is open; it cannot report "spawn
# granted, hand-off denied" independently. Exercise the feasible degrade:
# remove the seed, answer n in a visible scratch pane, assert session/plugin
# survive and no new PANIC IN PLUGIN line appears. Denied aggregate leaves
# terminal visible + plugin suppressed/alive; no gated host call runs.
SES3="htoggled$$"
HOST3="htoggled-host$$"
cleanup3() {
  tmux kill-session -t "$HOST3" 2>/dev/null || true
  zellij kill-session "$SES3" 2>/dev/null || true
  sleep 1
  zellij delete-session "$SES3" --force >/dev/null 2>&1 || true
}
trap 'cleanup; cleanup2; cleanup3' EXIT
# Remove exactly this wasm's seeded block so the permission UI appears.
awk -v wasm="$WASM" 'BEGIN{skip=0} $0 == "\"" wasm "\" {" {skip=1; next} skip && /^\}/ {skip=0; next} !skip {print}' "$PERM_FILE" > "$PERM_FILE.tmp" && mv "$PERM_FILE.tmp" "$PERM_FILE"
LOG_FILE=""
for d in "${TMPDIR:-/tmp}" /tmp /var/folders/*/*/T; do
  [ -f "$d/zellij-$(id -u)/zellij-log/zellij.log" ] && { LOG_FILE="$d/zellij-$(id -u)/zellij-log/zellij.log"; break; }
done
PANIC_BEFORE=0
[ -n "$LOG_FILE" ] && PANIC_BEFORE="$(grep -c "PANIC IN PLUGIN" "$LOG_FILE" 2>/dev/null || true)"
tmux new-session -d -s "$HOST3" -x 180 -y 45 "zellij --config $CFG -s $SES3"
for try in 1 2 3 4 5 6 7 8 9 10; do
  zellij list-sessions 2>/dev/null | grep -q "$SES3" && break
  sleep 1
done
sleep 2
tmux send-keys -t "$HOST3" Escape; sleep 1 # dismiss About Zellij tip
# Cold invoke opens the permission UI in this visible plugin pane.
tmux send-keys -t "$HOST3" F6; sleep 2
tmux send-keys -t "$HOST3" n; sleep 3
DENIED_LAYOUT="$(zellij -s "$SES3" action dump-layout 2>/dev/null || true)"
DENIED_PANE_RC=1; echo "$DENIED_LAYOUT" | grep -q "harpoon.wasm" && DENIED_PANE_RC=0
PANIC_AFTER="$PANIC_BEFORE"
[ -n "$LOG_FILE" ] && PANIC_AFTER="$(grep -c "PANIC IN PLUGIN" "$LOG_FILE" 2>/dev/null || true)"
assert "S11 aggregate permission denial leaves harpoon pane/session alive" "$DENIED_PANE_RC"
assert "S11 aggregate permission denial adds no plugin panic (before=$PANIC_BEFORE after=$PANIC_AFTER)" \
  "$([ "$PANIC_AFTER" -eq "$PANIC_BEFORE" ]; echo $?)"

say "----"
say "scenarios: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
