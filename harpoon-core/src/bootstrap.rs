//! Respawn state hand-off decision logic.
//!
//! The `toggle` pipe's respawn branch hands the outgoing instance's
//! in-memory bookmark state directly to its successor (a `bootstrap_store`
//! pipe routed by destination plugin id), instead of leaving the successor
//! to race the async disk load. This module owns every DECISION in that
//! flow (Constitution I — core decides, shim executes):
//!
//! - payload codec (persistence-v2-envelope-shaped + session name + the
//!   sender-handled CLI pipe id);
//! - adoption-vs-disk precedence (`bootstrap_arrival_decision`,
//!   `disk_load_decision`);
//! - duplicate/stale toggle tolerance (`DuplicateToggleGuard`) — probe
//!   evidence 2026-07-13 (`evidence/spike-log-excerpt.txt`): the in-flight
//!   CLI invocation pipe is RE-DELIVERED to the successor ~380ms after
//!   load with the SAME pipe uuid, so identity — not timing — is the
//!   deterministic key;
//! - the destructive save guard (`shrinking save` detection + readiness)
//!   for `reorder.destructive-save-guard`.
//!
//! ACs: `pane-pipe-api.respawn-state-hand-off`,
//! `pane-pipe-api.duplicate-toggle-delivery-tolerance`,
//! `reorder.destructive-save-guard`.

use serde::{Deserialize, Serialize};

use crate::bookmark::PaneBookmark;

/// Pipe name for the hand-off message (never a broadcast pipe).
pub const BOOTSTRAP_PIPE_NAME: &str = "bootstrap_store";

/// Payload schema version — locked to the persistence v2 envelope shape.
pub const BOOTSTRAP_VERSION: u8 = 2;

/// The hand-off payload. Field shape deliberately mirrors the on-disk v2
/// envelope (`{version, bookmarks}`) so one serializer discipline covers
/// both (proposal Q1); `bookmarks` carries `PaneBookmark.id`, so the
/// successor — same zellij session, same pane ids — resolves by stable id
/// without any title matching (`reorder.restore-identity-tracks-live-panes`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapPayload {
    pub version: u8,
    pub bookmarks: Vec<PaneBookmark>,
    /// Sender's session name (the successor may not have seen
    /// `SessionUpdate` yet; it needs the name to address the disk file).
    #[serde(default)]
    pub session_name: Option<String>,
    /// CLI pipe uuid of the invocation the SENDER already handled (the
    /// toggle that triggered this respawn). zellij re-delivers the
    /// still-open CLI pipe to the successor; the successor must ignore
    /// exactly one toggle from this source.
    #[serde(default)]
    pub handled_cli_pipe: Option<String>,
}

/// Serialize a payload. `None` only on serializer failure (never expected
/// for this shape; callers degrade to the disk-load path).
pub fn encode_bootstrap(payload: &BootstrapPayload) -> Option<String> {
    serde_json::to_string(payload).ok()
}

/// Parse a payload. Deny-safe: any malformed/foreign/wrong-version input
/// yields `None` (the successor then keeps today's disk-load path). Pure —
/// no host calls — because the bootstrap arrives PRE-GRANT (probe rider a).
pub fn decode_bootstrap(raw: &str) -> Option<BootstrapPayload> {
    let payload: BootstrapPayload = serde_json::from_str(raw).ok()?;
    if payload.version != BOOTSTRAP_VERSION {
        return None;
    }
    Some(payload)
}

/// Store-population lifecycle flags (pure state, owned by the shim's
/// `State`, mutated only on the events named here).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreBootState {
    /// A bootstrap payload has been adopted this instance.
    pub adopted: bool,
    /// The disk load has RESOLVED — success OR explicit failure (an
    /// unresolved load is the race window this change closes).
    pub disk_resolved: bool,
    /// The user has mutated the in-memory store this instance
    /// (add/delete/reorder/freeze) — newer than any bootstrap or disk data.
    pub mutated: bool,
    /// At least one non-empty pane manifest has been observed.
    pub manifest_seen: bool,
}

/// What to do with an arriving `bootstrap_store` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdoptDecision {
    /// Replace the in-memory store with the payload.
    Adopt,
    /// Drop the payload (something newer already exists in memory).
    Ignore,
}

/// AC `pane-pipe-api.respawn-state-hand-off`: the payload is the sender's
/// LIVE state at send time — newer than disk, older than any user mutation
/// the successor has already applied. Adopt unless the user got there
/// first; a resolved disk load does NOT outrank the payload.
pub fn bootstrap_arrival_decision(state: &StoreBootState) -> AdoptDecision {
    if state.mutated {
        AdoptDecision::Ignore
    } else {
        AdoptDecision::Adopt
    }
}

/// What to do with a disk-load result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskLoadDecision {
    /// Cold boot, nothing newer in memory — use the disk contents (today's
    /// behavior, unchanged).
    UseDisk,
    /// The user mutated an un-bootstrapped store before the load resolved —
    /// keep the mutations AND append disk bookmarks missing from memory
    /// (a late disk result reconciles; it never clobbers newer mutations).
    MergeMissing,
    /// A bootstrap payload was adopted — preserve it verbatim (the sender's
    /// live state is newer than disk), but consume the late disk result as
    /// the persistence baseline. This is reconciliation, not a dropped
    /// result: it completes disk readiness and seeds shrink detection while
    /// never replacing/adjoining stale disk rows into newer memory.
    ReconcileBaseline,
}

/// AC `pane-pipe-api.respawn-state-hand-off` (late-disk-load scenario).
pub fn disk_load_decision(state: &StoreBootState) -> DiskLoadDecision {
    if state.adopted {
        DiskLoadDecision::ReconcileBaseline
    } else if state.mutated {
        DiskLoadDecision::MergeMissing
    } else {
        DiskLoadDecision::UseDisk
    }
}

/// Stable identity key for shrink/merge comparisons: the pane id when the
/// bookmark has resolved, else the `(tab_name, pane_title)` fallback pair.
fn identity_key(b: &PaneBookmark) -> (Option<u32>, &str, &str) {
    match b.id {
        Some(id) => (Some(id), "", ""),
        None => (None, b.tab_name.as_str(), b.pane_title.as_str()),
    }
}

/// Append entries of `disk` whose identity is absent from `memory`
/// (the `MergeMissing` reconciliation). Appended entries get
/// `index = None` (append-on-resolve semantics — never disturb the
/// user's current layout).
pub fn merge_missing(memory: &mut Vec<PaneBookmark>, disk: &[PaneBookmark]) {
    for d in disk {
        if !memory.iter().any(|m| identity_key(m) == identity_key(d)) {
            let mut b = d.clone();
            b.index = None;
            memory.push(b);
        }
    }
}

/// One-shot guard against the re-delivered in-flight invocation pipe.
///
/// Armed from the adopted payload's `handled_cli_pipe`; disarms after the
/// first match so a genuine later invoke from the same CLI source (unlikely
/// but possible) is honored. Keybind-sourced pipes carry no uuid and are
/// never suppressed (probe evidence covers CLI re-delivery only).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuplicateToggleGuard {
    handled_cli_pipe: Option<String>,
}

impl DuplicateToggleGuard {
    pub fn arm(&mut self, handled_cli_pipe: Option<String>) {
        self.handled_cli_pipe = handled_cli_pipe;
    }

    /// AC `pane-pipe-api.duplicate-toggle-delivery-tolerance`: `true` iff
    /// this toggle is the stale re-delivery of the pipe the sender already
    /// handled. The caller must still release the CLI client (the pipe is
    /// otherwise left blocked) but MUST NOT hide the menu.
    pub fn is_stale_toggle(&mut self, source_cli_uuid: Option<&str>) -> bool {
        match (self.handled_cli_pipe.as_deref(), source_cli_uuid) {
            (Some(handled), Some(source)) if handled == source => {
                self.handled_cli_pipe = None; // one-shot
                true
            }
            _ => false,
        }
    }
}

/// AC `reorder.destructive-save-guard`: a shrinking save is one that drops
/// an identity present in the last persisted state.
pub fn is_shrinking_save(last_persisted: &[PaneBookmark], candidate: &[PaneBookmark]) -> bool {
    last_persisted
        .iter()
        .any(|p| !candidate.iter().any(|c| identity_key(c) == identity_key(p)))
}

/// AC `reorder.destructive-save-guard`: shrinking saves are allowed only
/// once the instance has observed BOTH a resolved disk load AND a pane
/// manifest. (An adopted bootstrap counts as the disk baseline — the
/// sender's disk was current at send time.)
pub fn shrinking_save_allowed(state: &StoreBootState) -> bool {
    // Frozen intent + reorder.destructive-save-guard require BOTH an
    // independently RESOLVED disk load and a FULL manifest. Adoption is a
    // usable render/bootstrap baseline, but never substitutes for the disk
    // readiness half of this destructive operation.
    state.disk_resolved && state.manifest_seen
}

/// Whether a PaneUpdate snapshot covers every tab position currently known
/// from TabUpdate. This is the strongest verifiable "full pane manifest"
/// signal zellij 0.44.3 exposes: PaneUpdate is a full snapshot by API shape,
/// and cross-tab completeness means every known tab has a manifest entry.
/// Empty tab lists/manifests are never ready.
pub fn manifest_covers_tabs(manifest_positions: &[usize], tab_positions: &[usize]) -> bool {
    !tab_positions.is_empty()
        && tab_positions
            .iter()
            .all(|tab| manifest_positions.iter().any(|seen| seen == tab))
}

/// Suppress user-visible menu rendering until one authoritative store source
/// has resolved. A successor's first ACTUAL render therefore contains the
/// adopted payload (cross-tab respawn) or disk-loaded state (cold boot),
/// never an empty intermediate list.
pub fn store_ready_to_render(state: &StoreBootState) -> bool {
    state.adopted || state.disk_resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bm(id: Option<u32>, tab: &str, title: &str, index: Option<u16>) -> PaneBookmark {
        PaneBookmark {
            tab_name: tab.to_string(),
            pane_title: title.to_string(),
            index,
            id,
        }
    }

    // ── codec: pane-pipe-api.respawn-state-hand-off ────────────────────────

    #[test]
    fn bootstrap_payload_round_trips_with_ids_and_session() {
        // pane-pipe-api.respawn-state-hand-off
        // reorder.restore-identity-tracks-live-panes (id-carry)
        let payload = BootstrapPayload {
            version: BOOTSTRAP_VERSION,
            bookmarks: vec![bm(Some(7), "work", "nvim", Some(0)), bm(None, "logs", "tail", None)],
            session_name: Some("workspace".into()),
            handled_cli_pipe: Some("eb176526-e2ba-4bd2-a60f-47e474a2e5a6".into()),
        };
        let encoded = encode_bootstrap(&payload).expect("encodes");
        let decoded = decode_bootstrap(&encoded).expect("decodes");
        assert_eq!(decoded, payload);
        assert_eq!(decoded.bookmarks[0].id, Some(7));
    }

    #[test]
    fn decode_rejects_garbage_and_wrong_version() {
        // pane-pipe-api.respawn-state-hand-off (deny-safe adoption)
        assert!(decode_bootstrap("not json").is_none());
        assert!(decode_bootstrap("{}").is_none());
        assert!(decode_bootstrap(r#"{"version":1,"bookmarks":[]}"#).is_none());
        // v2 with defaults for optional fields parses.
        assert!(decode_bootstrap(r#"{"version":2,"bookmarks":[]}"#).is_some());
    }

    // ── adoption precedence: pane-pipe-api.respawn-state-hand-off ──────────

    #[test]
    fn bootstrap_adopted_while_disk_unresolved() {
        let state = StoreBootState::default();
        assert_eq!(bootstrap_arrival_decision(&state), AdoptDecision::Adopt);
    }

    #[test]
    fn bootstrap_adopted_even_after_disk_resolved_when_unmutated() {
        // Payload is the sender's LIVE state — newer than disk.
        let state = StoreBootState { disk_resolved: true, ..Default::default() };
        assert_eq!(bootstrap_arrival_decision(&state), AdoptDecision::Adopt);
    }

    #[test]
    fn bootstrap_ignored_after_user_mutation() {
        let state = StoreBootState { mutated: true, ..Default::default() };
        assert_eq!(bootstrap_arrival_decision(&state), AdoptDecision::Ignore);
    }

    #[test]
    fn late_disk_load_never_clobbers_adopted_store() {
        // pane-pipe-api.respawn-state-hand-off (late disk load scenario)
        let state = StoreBootState { adopted: true, ..Default::default() };
        assert_eq!(
            disk_load_decision(&state),
            DiskLoadDecision::ReconcileBaseline
        );
        let state = StoreBootState { adopted: true, mutated: true, ..Default::default() };
        assert_eq!(
            disk_load_decision(&state),
            DiskLoadDecision::ReconcileBaseline
        );
    }

    #[test]
    fn cold_boot_disk_load_used_verbatim() {
        let state = StoreBootState::default();
        assert_eq!(disk_load_decision(&state), DiskLoadDecision::UseDisk);
    }

    #[test]
    fn disk_load_after_early_mutation_merges_without_clobbering() {
        let state = StoreBootState { mutated: true, ..Default::default() };
        assert_eq!(disk_load_decision(&state), DiskLoadDecision::MergeMissing);

        let mut memory = vec![bm(Some(9), "new", "added-by-user", Some(0))];
        let disk = vec![
            bm(Some(9), "new", "added-by-user", Some(0)), // same identity
            bm(None, "old", "from-disk", Some(1)),
        ];
        merge_missing(&mut memory, &disk);
        assert_eq!(memory.len(), 2);
        assert_eq!(memory[0].pane_title, "added-by-user"); // untouched
        assert_eq!(memory[1].pane_title, "from-disk");
        assert_eq!(memory[1].index, None); // append-on-resolve, not placed
    }

    // ── duplicate toggle: pane-pipe-api.duplicate-toggle-delivery-tolerance ─

    #[test]
    fn stale_redelivered_pipe_is_suppressed_once() {
        let mut guard = DuplicateToggleGuard::default();
        guard.arm(Some("uuid-1".into()));
        assert!(guard.is_stale_toggle(Some("uuid-1")));
        // One-shot: an identical later source is honored.
        assert!(!guard.is_stale_toggle(Some("uuid-1")));
    }

    #[test]
    fn fresh_sources_are_never_suppressed() {
        let mut guard = DuplicateToggleGuard::default();
        guard.arm(Some("uuid-1".into()));
        assert!(!guard.is_stale_toggle(Some("uuid-2"))); // different CLI client
        assert!(!guard.is_stale_toggle(None)); // keybind source (no uuid)
        // Non-matches do not disarm the guard.
        assert!(guard.is_stale_toggle(Some("uuid-1")));
    }

    #[test]
    fn unarmed_guard_suppresses_nothing() {
        let mut guard = DuplicateToggleGuard::default();
        assert!(!guard.is_stale_toggle(Some("uuid-1")));
        assert!(!guard.is_stale_toggle(None));
    }

    // ── save guard: reorder.destructive-save-guard ──────────────────────────

    #[test]
    fn shrinking_save_detected_by_identity() {
        let last = vec![bm(Some(1), "a", "t1", Some(0)), bm(Some(2), "b", "t2", Some(1))];
        // Dropping id=2 shrinks.
        assert!(is_shrinking_save(&last, &[bm(Some(1), "a", "t1", Some(0))]));
        // Title refresh on the SAME id is not a shrink.
        let refreshed = vec![bm(Some(1), "a", "new-title", Some(0)), bm(Some(2), "b", "t2", Some(1))];
        assert!(!is_shrinking_save(&last, &refreshed));
        // Additive is not a shrink.
        let mut grown = last.clone();
        grown.push(bm(Some(3), "c", "t3", Some(2)));
        assert!(!is_shrinking_save(&last, &grown));
        // Reorder (index changes only) is not a shrink.
        let reordered = vec![bm(Some(2), "b", "t2", Some(0)), bm(Some(1), "a", "t1", Some(1))];
        assert!(!is_shrinking_save(&last, &reordered));
    }

    #[test]
    fn shrink_forbidden_until_disk_and_manifest_observed() {
        // reorder.destructive-save-guard (early window)
        assert!(!shrinking_save_allowed(&StoreBootState::default()));
        assert!(!shrinking_save_allowed(&StoreBootState {
            disk_resolved: true,
            ..Default::default()
        }));
        assert!(!shrinking_save_allowed(&StoreBootState {
            manifest_seen: true,
            ..Default::default()
        }));
        // reorder.destructive-save-guard (pruning resumes)
        assert!(shrinking_save_allowed(&StoreBootState {
            disk_resolved: true,
            manifest_seen: true,
            ..Default::default()
        }));
        // Adoption never substitutes for an independently-resolved disk
        // load (frozen intent: BOTH resolved disk + full manifest).
        assert!(!shrinking_save_allowed(&StoreBootState {
            adopted: true,
            manifest_seen: true,
            ..Default::default()
        }));
    }

    #[test]
    fn full_manifest_requires_coverage_of_every_known_tab() {
        // reorder.destructive-save-guard (partial-manifest window)
        assert!(!manifest_covers_tabs(&[], &[0]));
        assert!(!manifest_covers_tabs(&[0], &[]));
        assert!(!manifest_covers_tabs(&[0], &[0, 1]));
        assert!(manifest_covers_tabs(&[0, 1], &[0, 1]));
        assert!(manifest_covers_tabs(&[2, 0, 1], &[0, 1, 2]));
    }

    #[test]
    fn first_render_waits_for_bootstrap_or_disk() {
        // pane-pipe-api.respawn-state-hand-off (no empty first render)
        assert!(!store_ready_to_render(&StoreBootState::default()));
        assert!(store_ready_to_render(&StoreBootState {
            adopted: true,
            ..Default::default()
        }));
        assert!(store_ready_to_render(&StoreBootState {
            disk_resolved: true,
            ..Default::default()
        }));
    }
}
