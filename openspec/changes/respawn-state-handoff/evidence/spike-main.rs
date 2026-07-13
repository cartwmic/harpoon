//! Spike: targeted bootstrap hand-off probe (throwaway — never shipped).
//!
//! Questions:
//!  Q1  Does `open_plugin_pane_floating` return `Some(PaneId::Plugin(id))`
//!      and is that id usable as `MessageToPlugin::destination_plugin_id`?
//!  Q2  Does a destination-id bootstrap pipe reach a JUST-spawned successor
//!      (still loading / just loaded), i.e. is it queued rather than dropped?
//!  Q3  Where does the bootstrap land in the successor's event order relative
//!      to load / PermissionRequestResult / first PaneUpdate?
//!
//! Protocol: CLI pipe "go" with payload = own wasm url. Parent spawns
//! successor with same url + SAME (empty) config, sends destination-id
//! bootstrap, then close_self(). All evidence via eprintln -> zellij.log.

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

#[derive(Default)]
struct Spike {
    own_id: u32,
    granted: bool,
    seq: u32,
    saw_pane_update: bool,
}

impl Spike {
    fn log(&mut self, msg: &str) {
        self.seq += 1;
        eprintln!("SPIKEHO id={} seq={} {}", self.own_id, self.seq, msg);
    }
}

register_plugin!(Spike);

impl ZellijPlugin for Spike {
    fn load(&mut self, _config: BTreeMap<String, String>) {
        self.own_id = get_plugin_ids().plugin_id;
        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::PaneUpdate,
        ]);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenTerminalsOrPlugins,
            PermissionType::MessageAndLaunchOtherPlugins,
            PermissionType::ReadCliPipes,
        ]);
        self.log("load");
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                self.granted = matches!(status, PermissionStatus::Granted);
                let g = self.granted;
                self.log(&format!("permission_result granted={}", g));
            }
            Event::PaneUpdate(_) => {
                if !self.saw_pane_update {
                    self.saw_pane_update = true;
                    self.log("first_pane_update");
                }
            }
            _ => {}
        }
        false
    }

    fn pipe(&mut self, msg: PipeMessage) -> bool {
        if matches!(msg.source, PipeSource::Cli(_)) {
            unblock_cli_pipe_input(&msg.name);
        }
        match msg.name.as_str() {
            "go" => {
                let src = format!("{:?}", msg.source);
                self.log(&format!("go_received source={}", src));
                let Some(url) = msg.payload.clone() else {
                    self.log("go_missing_payload");
                    return false;
                };
                if !self.granted {
                    self.log("go_but_not_granted_ABORT");
                    return false;
                }
                // Q1: spawn successor, capture returned pane id.
                let spawned =
                    open_plugin_pane_floating(&url, BTreeMap::new(), None, BTreeMap::new());
                self.log(&format!("spawn_returned {:?}", spawned));
                if let Some(PaneId::Plugin(new_id)) = spawned {
                    // Q2: targeted bootstrap to the fresh id.
                    let m = MessageToPlugin::new("bootstrap_store")
                        .with_destination_plugin_id(new_id)
                        .with_payload(format!("store-from-{}", self.own_id));
                    pipe_message_to_plugin(m);
                    self.log(&format!("bootstrap_sent dest={}", new_id));
                } else {
                    self.log("spawn_no_plugin_pane_id");
                }
                close_self();
                self.log("close_self_called");
            }
            "bootstrap_store" => {
                // Q3: event-order evidence is the seq number + surrounding lines.
                self.log(&format!(
                    "BOOTSTRAP_RECEIVED payload={:?} source={:?} granted={} saw_pane_update={}",
                    msg.payload, msg.source, self.granted, self.saw_pane_update
                ));
            }
            _ => {}
        }
        false
    }
}
