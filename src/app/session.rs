//! Session lifecycle and bounded, debounced recovery checkpoints.
use super::safety::SessionKey;
use super::*;
use markora::recovery::{Session, SessionStore, SessionWriter};
use std::time::{Duration, Instant};

impl Markora {
    pub(super) fn initialize_session(&mut self) {
        let result = (|| -> Result<(), String> {
            let store = SessionStore::open(self.storage.dir.clone()).map_err(|e| e.to_string())?;
            if let Some(session) = store
                .load()
                .map_err(|e| format!("Recovery file preserved: {e}"))?
            {
                let folder = session.folder.clone();
                let (workspace, errors) =
                    session.restore(self.prefs.view_mode, self.webview.is_none());
                self.workspace = workspace;
                if let Some(folder) = folder {
                    self.open_folder(folder);
                }
                if errors.is_empty() {
                    self.notify("Restored your previous tabs and drafts");
                } else {
                    self.fail(format!(
                        "Recovered drafts; some files could not be reopened: {}",
                        errors.join("; ")
                    ));
                }
            }
            self.safety.writer = Some(SessionWriter::start(store));
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(format!("Session recovery unavailable: {error}"));
        }
    }

    pub(super) fn checkpoint_session(&mut self) {
        if self.safety.writer.is_none() {
            return;
        }
        let key = SessionKey {
            tabs: self
                .workspace
                .tabs
                .iter()
                .map(|d| {
                    (
                        d.id,
                        d.revision,
                        d.path.clone(),
                        d.view_mode,
                        d.dirty(),
                        d.disk_version,
                    )
                })
                .collect(),
            active: self.workspace.active,
            folder: self.folder.clone(),
        };
        if self.safety.session_key.as_ref() != Some(&key) {
            self.safety.session_key = Some(key);
            self.safety.changed_at = Instant::now();
            self.safety.pending_since.get_or_insert_with(Instant::now);
        }
        let results: Vec<_> = self
            .safety
            .writer
            .as_ref()
            .unwrap()
            .results
            .try_iter()
            .collect();
        for result in results {
            if let Err(error) = result {
                self.fail(format!("Draft recovery could not be saved: {error}"));
                self.safety.pending_since.get_or_insert_with(Instant::now);
            }
        }
        if self.safety.pending_since.is_some_and(|start| {
            start.elapsed() >= Duration::from_secs(2)
                || self.safety.changed_at.elapsed() >= Duration::from_millis(750)
        }) {
            let session = Session::capture(&self.workspace, self.folder.clone(), &[]);
            if self.safety.writer.as_ref().unwrap().schedule(session) {
                self.safety.pending_since = None;
            }
        }
    }

    pub(super) fn flush_session(&mut self) -> bool {
        let Some(writer) = &self.safety.writer else {
            return true;
        };
        let session =
            Session::capture(&self.workspace, self.folder.clone(), &self.safety.discarded);
        match writer.flush(session) {
            Ok(()) => true,
            Err(error) => {
                self.fail(format!(
                    "Could not save the session before closing: {error}"
                ));
                false
            }
        }
    }
}
