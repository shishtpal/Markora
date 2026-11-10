//! Native command dispatch and the shared unsaved-changes state machine.
use super::*;
use markora::{
    model::ViewMode,
    protocol::{ExportFormat, HtmlStyle, UnsavedChoice},
};

impl Markora {
    pub(super) fn run_command(&mut self, command: &str) {
        let id = self.workspace.doc().id;
        match command {
            "new" | "new-tab" => self.workspace.add(String::new(), self.prefs.view_mode),
            "open" => self.pick_file(),
            "open-folder" => self.pick_folder(),
            "save" => {
                self.save_document(id, false);
            }
            "save-as" => {
                self.save_document(id, true);
            }
            "export-html" => self.export_document(ExportFormat::Html, HtmlStyle::Current),
            "export-pdf" => self.export_document(ExportFormat::Pdf, HtmlStyle::Current),
            "export-rtf" => self.export_document(ExportFormat::Rtf, HtmlStyle::Current),
            "export-txt" => self.export_document(ExportFormat::Txt, HtmlStyle::Current),
            "print" => self.prepare_print(None),
            "save-all" => {
                let ids: Vec<_> = self
                    .workspace
                    .tabs
                    .iter()
                    .filter(|d| d.dirty())
                    .map(|d| d.id)
                    .collect();
                for id in ids {
                    if !self.save_document(id, false) {
                        return;
                    }
                }
                self.notify("All documents saved");
            }
            "close" | "close-tab" => self.request_close(CloseAction::Tab(id)),
            "close-all" => self.request_close(CloseAction::All),
            "exit" => self.request_close(CloseAction::Exit),
            "tab-next" | "tab-prev" => {
                let count = self.workspace.tabs.len();
                self.workspace.active = (self.workspace.active
                    + if command == "tab-prev" { count - 1 } else { 1 })
                    % count;
            }
            "view-live" | "view-source" | "view-split" => {
                let view = match command {
                    "view-source" => ViewMode::Source,
                    "view-split" => ViewMode::Split,
                    _ => ViewMode::Live,
                };
                self.workspace.doc_mut().set_view_mode(view);
                self.prefs.view_mode = view;
                self.persist_prefs();
            }
            "toggle-files" => {
                self.prefs.show_files = !self.prefs.show_files;
                self.persist_prefs();
            }
            "toggle-outline" => {
                self.prefs.show_outline = !self.prefs.show_outline;
                self.persist_prefs();
            }
            "toggle-focus" => {
                self.prefs.focus_mode = !self.prefs.focus_mode;
                self.persist_prefs();
            }
            "toggle-theme" => {
                self.prefs.color_mode = if self.prefs.color_mode == "dark" {
                    "light"
                } else {
                    "dark"
                }
                .into();
                self.persist_prefs();
            }
            "md-theme-next" => {
                let index = Prefs::THEMES
                    .iter()
                    .position(|t| *t == self.prefs.md_theme)
                    .unwrap_or(0);
                self.prefs.md_theme = Prefs::THEMES[(index + 1) % Prefs::THEMES.len()].into();
                self.persist_prefs();
            }
            "refresh-folder" => {
                if let Some(folder) = &self.folder {
                    self.open_folder(folder.clone());
                }
            }
            "folder-up" => {
                if let Some(parent) = self.folder.as_ref().and_then(|p| p.parent()) {
                    self.open_folder(parent.into());
                }
            }
            "clear-recents" => {
                self.recents.clear();
                if let Err(error) = self.storage.write("recents.json", &self.recents) {
                    self.fail(error.to_string());
                }
            }
            "about" => self.notify("Markora · Markdown editor · Rust + WebView2"),
            unknown => self.fail(format!("Unknown command: {unknown}")),
        }
        self.needs_sync = true;
    }

    pub(super) fn request_close(&mut self, action: CloseAction) {
        if self.pending.is_some() {
            return;
        }
        let documents = self
            .workspace
            .tabs
            .iter()
            .filter(|d| {
                d.dirty()
                    && match action {
                        CloseAction::Tab(id) => d.id == id,
                        _ => true,
                    }
            })
            .map(|d| d.id)
            .collect::<VecDeque<_>>();
        if documents.is_empty() {
            self.complete_close(action);
        } else {
            self.pending = Some(PendingClose { action, documents });
        }
        self.needs_sync = true;
    }

    pub(super) fn resolve_unsaved(&mut self, choice: UnsavedChoice) {
        let Some(mut pending) = self.pending.take() else {
            return;
        };
        match choice {
            UnsavedChoice::Cancel => {}
            UnsavedChoice::Save | UnsavedChoice::Discard => {
                if let Some(id) = pending.documents.front()
                    && matches!(choice, UnsavedChoice::Save)
                    && !self.save_document(*id, false)
                {
                    self.pending = Some(pending);
                    self.needs_sync = true;
                    return;
                }
                pending.documents.pop_front();
                if pending.documents.is_empty() {
                    self.complete_close(pending.action);
                } else {
                    self.pending = Some(pending);
                }
            }
        }
        self.needs_sync = true;
    }

    fn complete_close(&mut self, action: CloseAction) {
        match action {
            CloseAction::Tab(id) => self.workspace.close(id, self.prefs.view_mode),
            CloseAction::All => {
                let ids: Vec<_> = self.workspace.tabs.iter().map(|d| d.id).collect();
                for id in ids {
                    self.workspace.close(id, self.prefs.view_mode);
                }
            }
            CloseAction::Exit => self.allow_close = true,
        }
    }
}
