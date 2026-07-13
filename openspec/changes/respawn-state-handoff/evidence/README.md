# Spike evidence — targeted bootstrap hand-off probe (2026-07-13)

Throwaway probe run during explore (originally `/tmp/spike-handoff/`,
preserved here per task 1.1; `/tmp` does not survive reboot). Scratch
zellij session inside tmux; never the live workspace.

Files:
- `spike-main.rs` / `spike-Cargo.toml` — probe plugin (zellij-tile 0.44.3,
  wasm32-wasip1 **bin** target; a cdylib build loads but exports no entry
  fn and dies with "could not find exported function").
- `spike-probe.sh` — harness: seeds permissions.kdl, loads the probe,
  sends the `go` pipe, collects `SPIKEHO` log lines, restores seeds.
- `spike-log-excerpt.txt` — the run cited by intent.md, verbatim.

What the excerpt proves (keyed to intent.md):
1. `spawn_returned Some(Plugin(5))` — `open_plugin_pane_floating` returns
   the successor's pane id (id 4 seq=5).
2. `bootstrap_sent dest=5` → id 5 `BOOTSTRAP_RECEIVED source=Plugin(4)`
   119ms after successor load — `destination_plugin_id` routing delivers
   across the successor's loading window (queued, not dropped).
3. `BOOTSTRAP_RECEIVED ... granted=false` — the bootstrap arrives
   **pre-grant** (before id 5's `permission_result granted=true`):
   adoption must be deny-safe pure state (probe rider a).
4. id 5 seq=5 `go_received` with the SAME Cli pipe uuid `eb176526…` the
   old instance already handled — the in-flight invocation pipe is
   **re-delivered to the successor** ~380ms later (probe rider b:
   duplicate-toggle tolerance).
5. Event order at successor: load → first `PaneUpdate` → bootstrap →
   grant → re-delivered pipe.
