//! Disk I/O wrapper around `harpoon_core::BookmarkStore`.
//!
//! Schema is the v2 envelope: `{ "version": 2, "bookmarks": [...] }`. v1
//! files (bare `Vec<PaneBookmark>` array) are read transparently and
//! re-saved as v2.
//!
//! See `openspec/changes/add-filter-and-jump-modes/design.md`:
//! - "Decision: Persistence schema v2 — envelope with single bookmarks Vec"
//! - "Decision: Persistence has_changed covers full persisted shape"
//! - "Decision: Schema version detection via JSON envelope shape"

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;

use harpoon_core::{clear_untrusted_pane_ids, BookmarkStore, PaneBookmark};

/// v2 on-disk envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedV2 {
    pub version: u8,
    pub bookmarks: Vec<PaneBookmark>,
}

#[derive(Debug)]
pub enum PersistenceError {
    LoadFromDiskFailed(String),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistenceError::LoadFromDiskFailed(e) => {
                write!(f, "Failed to load session from disk: {e}")
            }
        }
    }
}

/// I/O wrapper. The actual `BookmarkStore` lives on `State` (so dispatch
/// handlers can mutate it via `&mut BookmarkStore`); this struct only
/// provides save/load mechanics.
#[derive(Default)]
pub struct Persistence {
    /// Last canonical bookmark baseline, used for `has_changed` and guarded
    /// shrink classification.
    last_saved_state: Option<PersistedV2>,
    /// A v1 bare array was loaded successfully. It is a known comparison
    /// baseline, but the next save must still rewrite the v2 envelope even
    /// when bookmark rows are otherwise unchanged.
    needs_v2_migration: bool,
}

impl Persistence {
    /// Path to the persistence directory (XDG-conformant; matches today's
    /// existing fork).
    fn data_dir_path(&self) -> String {
        "${XDG_DATA_HOME:-$HOME/.local/share}/zellij-harpoon".to_string()
    }

    fn session_file_path(&self, session_name: &Option<String>) -> Option<String> {
        let session = session_name.as_ref()?;
        Some(format!("{}/{}.json", self.data_dir_path(), session))
    }

    /// Kick off an async `cat` to read the session file. The result arrives
    /// as `Event::RunCommandResult` with `context["source"] == "load"`,
    /// processed by [`Persistence::on_load_command`].
    pub fn load_from_disk(&self, session_name: &Option<String>) {
        let Some(file_path) = self.session_file_path(session_name) else {
            return;
        };
        let cmd = format!("cat {file_path} 2>/dev/null || echo '[]'");
        let mut context = BTreeMap::new();
        context.insert("source".to_string(), "load".to_string());
        run_command(&["sh", "-c", &cmd], context);
    }

    /// Process the result of `load_from_disk`'s `cat` command. Tries v2
    /// envelope first, falls back to v1 bare array. Populates `store`.
    pub fn on_load_command(
        &mut self,
        store: &mut BookmarkStore,
        content: &str,
    ) -> Result<(), PersistenceError> {
        // Try v2 envelope first.
        if let Ok(mut v2) = serde_json::from_str::<PersistedV2>(content) {
            // Pane ids are session-generation-local. A disk file can outlive
            // the generation and reuse an id for an unrelated pane; cold
            // restore MUST use the refreshed fallback identity instead.
            clear_untrusted_pane_ids(&mut v2.bookmarks);
            store.bookmarks = v2.bookmarks.clone();
            self.last_saved_state = Some(v2);
            self.needs_v2_migration = false;
            // pane_id_to_bookmark_idx stays empty until restore_round resolves.
            return Ok(());
        }
        // Fall back to v1 bare array.
        match serde_json::from_str::<Vec<PaneBookmark>>(content) {
            Ok(mut bookmarks) => {
                // Assign indices in array order so v1 files inherit positions.
                for (i, b) in bookmarks.iter_mut().enumerate() {
                    if b.index.is_none() {
                        b.index = Some(i as u16);
                    }
                }
                clear_untrusted_pane_ids(&mut bookmarks);
                store.bookmarks = bookmarks.clone();
                // A successfully parsed v1 file is a KNOWN baseline. Keep
                // its rows for immediate additive classification; the next
                // changed save still writes the canonical v2 envelope.
                self.last_saved_state = Some(PersistedV2 {
                    version: 2,
                    bookmarks,
                });
                self.needs_v2_migration = true;
                Ok(())
            }
            Err(e) => Err(PersistenceError::LoadFromDiskFailed(e.to_string())),
        }
    }

    /// Parse session-file content WITHOUT touching any state — v2 envelope
    /// first, v1 bare-array fallback (same detection as `on_load_command`).
    /// Used by the `MergeMissing` disk reconciliation
    /// (pane-pipe-api.respawn-state-hand-off).
    pub fn parse_content(content: &str) -> Option<(Vec<PaneBookmark>, bool)> {
        let (mut bookmarks, needs_v2_migration) =
            if let Ok(v2) = serde_json::from_str::<PersistedV2>(content) {
                (v2.bookmarks, false)
            } else {
                (
                    serde_json::from_str::<Vec<PaneBookmark>>(content).ok()?,
                    true,
                )
            };
        clear_untrusted_pane_ids(&mut bookmarks);
        Some((bookmarks, needs_v2_migration))
    }

    /// Seed the last-persisted baseline without a disk write. Used at
    /// bootstrap adoption: the sender's disk was current at send time, so
    /// the adopted payload IS the persisted state — giving the destructive
    /// save guard a baseline to compare against
    /// (reorder.destructive-save-guard).
    pub fn set_baseline(&mut self, bookmarks: Vec<PaneBookmark>) {
        self.set_disk_baseline(bookmarks, false);
    }

    /// Seed a parsed disk baseline while preserving v1 format provenance.
    /// Reconciliation must not erase mandatory next-save migration.
    pub fn set_disk_baseline(&mut self, bookmarks: Vec<PaneBookmark>, needs_v2_migration: bool) {
        self.last_saved_state = Some(PersistedV2 {
            version: 2,
            bookmarks,
        });
        self.needs_v2_migration = needs_v2_migration;
    }

    /// The last known persisted bookmark list (`None` until a disk load
    /// resolves, a save lands, or an adopted baseline is seeded).
    pub fn last_persisted(&self) -> Option<&[PaneBookmark]> {
        self.last_saved_state
            .as_ref()
            .map(|s| s.bookmarks.as_slice())
    }

    /// True iff the current `store.bookmarks` differs from the last
    /// successfully-saved envelope.
    pub fn has_changed(&self, store: &BookmarkStore) -> bool {
        if self.needs_v2_migration {
            return true;
        }
        let candidate = PersistedV2 {
            version: 2,
            bookmarks: store.bookmarks.clone(),
        };
        match &self.last_saved_state {
            Some(prev) => &candidate != prev,
            None => !store.bookmarks.is_empty(), // first save threshold
        }
    }

    /// Write the current `store.bookmarks` to disk if it differs from the
    /// last saved snapshot.
    pub fn save_if_changed(&mut self, store: &BookmarkStore, session_name: &Option<String>) {
        if !self.has_changed(store) {
            return;
        }
        self.save_to_disk(store, session_name);
    }

    /// Write the current `store.bookmarks` directly.
    pub fn save_to_disk(&mut self, store: &BookmarkStore, session_name: &Option<String>) {
        let Some(file_path) = self.session_file_path(session_name) else {
            return;
        };
        let envelope = PersistedV2 {
            version: 2,
            bookmarks: store.bookmarks.clone(),
        };
        let json = serde_json::to_string(&envelope).unwrap_or_else(|_| "[]".to_string());
        let cmd = format!(
            "mkdir -p {} && printf '%s' \"$1\" > {}",
            self.data_dir_path(),
            file_path,
        );
        let mut context = BTreeMap::new();
        context.insert("source".to_string(), "save".to_string());
        run_command(&["sh", "-c", &cmd, "_", &json], context);

        self.last_saved_state = Some(envelope);
        self.needs_v2_migration = false;
    }
}
