//! Resolve external edits and drive the native conflict dialog.
use super::*;
use markora::{file_io::FileVersion, model::Document, protocol::ConflictChoice};

impl Markora {
    pub(super) fn resolve_conflict(&mut self, choice: ConflictChoice) {
        let Some(conflict) = self.safety.conflict.take() else {
            return;
        };
        self.needs_sync = true;
        let Some(index) = self
            .workspace
            .tabs
            .iter()
            .position(|d| d.id == conflict.doc_id)
        else {
            return;
        };
        let Some(path) = self.workspace.tabs[index].path.clone() else {
            return;
        };
        let result = (|| -> Result<(), String> {
            match choice {
                ConflictChoice::Reload => {
                    let old = &self.workspace.tabs[index];
                    let prepared = storage::prepare_file(&path, old.view_mode, false)?;
                    let mut doc = Document::from_prepared(old.id, path, old.view_mode, prepared);
                    doc.revision = old.revision + 1;
                    doc.rendered_revision = doc.revision;
                    self.workspace.tabs[index] = doc;
                    self.synced_doc = None;
                    self.client_known_markdown = None;
                    self.fallback_buf = None;
                    self.notify("Reloaded the file from disk");
                    self.advance_conflict_close(conflict.doc_id);
                }
                ConflictChoice::Keep => {
                    let bytes = match std::fs::read(&path) {
                        Ok(bytes) => Some(bytes),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                        Err(e) => return Err(e.to_string()),
                    };
                    let version = bytes.as_deref().map(FileVersion::of).unwrap_or_default();
                    if version != conflict.version {
                        self.show_conflict(conflict.doc_id, version, conflict.saving);
                        self.notify(
                            "The file changed again. Review the conflict before continuing.",
                        );
                        return Ok(());
                    }
                    let doc = &mut self.workspace.tabs[index];
                    // Keep Mine acknowledges only the observed disk version, leaving a dirty draft.
                    doc.saved_markdown = Some(
                        bytes
                            .and_then(|b| String::from_utf8(b).ok())
                            .unwrap_or_default()
                            .trim_start_matches('\u{feff}')
                            .into(),
                    );
                    doc.disk_version = version;
                    self.notify("Kept your draft. Save to replace the reviewed disk version.");
                    if conflict.saving {
                        self.resume_conflict_save(conflict.doc_id, false);
                    }
                }
                ConflictChoice::SaveAs => self.resume_conflict_save(conflict.doc_id, true),
                ConflictChoice::Cancel => {
                    self.safety
                        .dismissed
                        .insert(conflict.doc_id, conflict.version);
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(format!("Could not resolve file change: {error}"));
        }
    }

    fn resume_conflict_save(&mut self, id: u64, save_as: bool) {
        if self.save_document(id, save_as) {
            self.advance_conflict_close(id);
        }
    }

    fn advance_conflict_close(&mut self, id: u64) {
        if self
            .pending
            .as_ref()
            .and_then(|p| p.documents.front())
            .copied()
            == Some(id)
        {
            // The draft is now clean; advance the close flow without writing again.
            self.resolve_unsaved(markora::protocol::UnsavedChoice::Discard);
        }
    }

    pub(super) fn conflict_ui(&mut self, ctx: &egui::Context) {
        let Some(conflict) = &self.safety.conflict else {
            return;
        };
        let message = if conflict.deleted {
            "This file was deleted outside Markora."
        } else {
            "This file changed outside Markora."
        };
        let path = conflict.path.clone();
        let deleted = conflict.deleted;
        let mut choice = None;
        egui::Modal::new(egui::Id::new("external-file-change")).show(ctx, |ui| {
            ui.heading("File changed on disk");
            ui.label(message);
            ui.label(path);
            ui.label("Reload replaces your draft. Keep Mine preserves it for saving.");
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!deleted, egui::Button::new("Reload"))
                    .clicked()
                {
                    choice = Some(ConflictChoice::Reload);
                }
                if ui.button("Keep Mine").clicked() {
                    choice = Some(ConflictChoice::Keep);
                }
                if ui.button("Save As…").clicked() {
                    choice = Some(ConflictChoice::SaveAs);
                }
                if ui.button("Later").clicked() {
                    choice = Some(ConflictChoice::Cancel);
                }
            });
        });
        if let Some(choice) = choice {
            self.resolve_conflict(choice);
        }
    }
}
