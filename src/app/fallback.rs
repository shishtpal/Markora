//! Small native source editor when a WebView2 child cannot be created.
use super::*;
use markora::protocol::UnsavedChoice;

impl Markora {
    pub(super) fn fallback_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut command = None;
        let active_doc = self.workspace.doc();
        let is_pdf = active_doc.kind == DocKind::Pdf;
        let active_id = active_doc.id;
        let pdf_pages = active_doc.pdf.as_ref().map(|p| p.page_count()).unwrap_or(0);
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (label, id) in [
                    ("New", "new-tab"),
                    ("Open", "open"),
                    ("Folder", "open-folder"),
                    ("Save", "save"),
                    ("Save as", "save-as"),
                    ("Files", "toggle-files"),
                    ("Outline", "toggle-outline"),
                    ("Theme", "toggle-theme"),
                ] {
                    let enabled = !(is_pdf && matches!(id, "save" | "save-as"));
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        command = Some(id);
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                let mut activate = None;
                let mut close = None;
                for doc in &self.workspace.tabs {
                    let label = format!("{}{}", doc.title(), if doc.dirty() { " •" } else { "" });
                    if ui
                        .selectable_label(doc.id == self.workspace.doc().id, label)
                        .clicked()
                    {
                        activate = Some(doc.id);
                    }
                    if ui.small_button("×").clicked() {
                        close = Some(doc.id);
                    }
                }
                if let Some(id) = activate {
                    self.workspace.activate(id);
                }
                if let Some(id) = close {
                    self.request_close(CloseAction::Tab(id));
                }
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            let mode = if is_pdf {
                "Native PDF viewer"
            } else {
                "Native source editor"
            };
            ui.label(format!("{} · {mode}", self.status));
        });
        if self.prefs.show_files && !self.prefs.focus_mode {
            egui::Panel::left("files")
                .resizable(true)
                .default_size(self.prefs.files_width)
                .show(ui, |ui| {
                    ui.heading("Files");
                    let mut open = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for entry in &self.entries {
                            if ui.button(&entry.name).clicked() {
                                open = Some(PathBuf::from(&entry.path));
                            }
                        }
                    });
                    if let Some(path) = open {
                        self.open_path(path);
                    }
                });
        }
        if self.prefs.show_outline && !self.prefs.focus_mode {
            egui::Panel::right("outline")
                .resizable(true)
                .default_size(self.prefs.outline_width)
                .show(ui, |ui| {
                    ui.heading(if is_pdf { "Pages" } else { "Outline" });
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if is_pdf {
                            let mut jump = None;
                            for page in 0..pdf_pages {
                                if ui.button(format!("Page {}", page + 1)).clicked() {
                                    jump = Some(page);
                                }
                            }
                            if let Some(page) = jump {
                                self.pdf_viewer.jump_to_page(active_id, page);
                            }
                        } else {
                            for heading in &self.workspace.doc().rendered.outline {
                                ui.label(&heading.title);
                            }
                        }
                    });
                });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if is_pdf {
                let doc = &self.workspace.tabs[self.workspace.active];
                self.pdf_viewer.ui(ui, doc);
                return;
            }
            let doc = self.workspace.doc_mut();
            let buf_stale = self
                .fallback_buf
                .as_ref()
                .is_none_or(|(id, rev, _)| *id != doc.id || *rev != doc.revision);
            if buf_stale {
                self.fallback_buf = Some((doc.id, doc.revision, doc.markdown.clone()));
            }
            let (_, rev, text) = self.fallback_buf.as_mut().expect("initialized");
            egui::ScrollArea::vertical().show(ui, |ui| {
                if ui
                    .add(
                        egui::TextEdit::multiline(text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(30),
                    )
                    .changed()
                {
                    let next_rev = doc.revision + 1;
                    *rev = next_rev;
                    doc.edit(text.clone(), next_rev);
                }
            });
        });
        if let Some(pending) = &self.pending
            && self.safety.conflict.is_none()
        {
            let title = pending
                .documents
                .front()
                .and_then(|id| self.workspace.tabs.iter().find(|d| d.id == *id))
                .map(|d| d.title())
                .unwrap_or_default();
            let mut choice = None;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&ctx, |ui| {
                    ui.label(format!("Save changes to {title}?"));
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            choice = Some(UnsavedChoice::Save);
                        }
                        if ui.button("Discard").clicked() {
                            choice = Some(UnsavedChoice::Discard);
                        }
                        if ui.button("Cancel").clicked() {
                            choice = Some(UnsavedChoice::Cancel);
                        }
                    });
                });
            if let Some(choice) = choice {
                self.resolve_unsaved(choice);
            }
        }
        if is_pdf && !ctx.egui_wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown)) {
                self.pdf_viewer.step_page(active_id, 1, pdf_pages);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp)) {
                self.pdf_viewer.step_page(active_id, -1, pdf_pages);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Equals)) {
                self.pdf_viewer.adjust_zoom(active_id, Some(1.15));
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Minus)) {
                self.pdf_viewer.adjust_zoom(active_id, Some(1.0 / 1.15));
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Num0)) {
                self.pdf_viewer.adjust_zoom(active_id, None);
            }
        }
        for (key, id) in [
            (egui::Key::S, "save"),
            (egui::Key::O, "open"),
            (egui::Key::N, "new-tab"),
            (egui::Key::W, "close-tab"),
        ] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, key)) {
                command = Some(id);
            }
        }
        if let Some(command) = command {
            self.run_command(command);
        }
    }
}
