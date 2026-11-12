//! Load only expanded directories; late results cannot replace another folder's tree.
use super::*;

fn find_entry<'a>(entries: &'a mut [FileEntry], path: &str) -> Option<&'a mut FileEntry> {
    for entry in entries {
        if entry.path == path && entry.is_folder {
            return Some(entry);
        }
        if let Some(children) = &mut entry.children
            && let Some(found) = find_entry(children, path)
        {
            return Some(found);
        }
    }
    None
}

impl Markora {
    pub(super) fn native_sidebar(&mut self, ui: &mut egui::Ui) {
        let mut open = None;
        let mut load = Vec::new();
        let active = self
            .workspace
            .doc()
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        ui.heading("Files");
        if ui.button("Open folder…").clicked() {
            self.pick_folder();
        }
        if let Some(folder) = &self.folder {
            ui.label(folder.display().to_string());
        }
        egui::ScrollArea::vertical()
            .id_salt("file-tree")
            .max_height(ui.available_height() * 0.65)
            .show(ui, |ui| {
                native_branch(ui, &self.entries, active.as_deref(), &mut open, &mut load);
                if self.entries.is_empty() {
                    ui.label("Open a folder to browse files.");
                }
            });
        ui.separator();
        egui::CollapsingHeader::new("Recent files")
            .default_open(true)
            .show(ui, |ui| {
                if ui.small_button("Clear").clicked() {
                    self.run_command("clear-recents");
                }
                egui::ScrollArea::vertical()
                    .id_salt("recent-files")
                    .show(ui, |ui| {
                        for recent in &self.recents {
                            if ui
                                .button(&recent.title)
                                .on_hover_text(recent.path.display().to_string())
                                .clicked()
                            {
                                open = Some(recent.path.clone());
                            }
                        }
                        if self.recents.is_empty() {
                            ui.label("Opened files appear here.");
                        }
                    });
            });
        for path in load {
            self.load_branch(path);
        }
        if let Some(path) = open {
            self.open_path(path);
        }
    }

    pub(super) fn load_branch(&mut self, path: String) {
        let Some(entry) = find_entry(&mut self.entries, &path) else {
            return;
        };
        if entry.loading || entry.children.is_some() {
            return;
        }
        entry.loading = true;
        entry.error = None;
        self.needs_sync = true;
        let generation = self.folder_generation;
        if let Some(ctx) = self.egui_ctx.clone() {
            let tx = self.bg_tx.clone();
            std::thread::spawn(move || {
                let result = storage::folder_entries(std::path::Path::new(&path))
                    .map_err(|error| error.to_string());
                let _ = tx.send(BgEvent::BranchLoaded {
                    path,
                    generation,
                    result,
                });
                ctx.request_repaint();
            });
        } else {
            let result = storage::folder_entries(std::path::Path::new(&path))
                .map_err(|error| error.to_string());
            self.finish_branch(&path, generation, result);
        }
    }

    pub(super) fn finish_branch(
        &mut self,
        path: &str,
        generation: u64,
        result: Result<Vec<FileEntry>, String>,
    ) {
        if generation != self.folder_generation {
            return;
        }
        let Some(entry) = find_entry(&mut self.entries, path) else {
            return;
        };
        entry.loading = false;
        match result {
            Ok(children) => {
                entry.children = Some(children);
                entry.error = None;
            }
            Err(error) => entry.error = Some(error),
        }
        self.needs_sync = true;
    }
}

fn native_branch(
    ui: &mut egui::Ui,
    entries: &[FileEntry],
    active: Option<&str>,
    open: &mut Option<PathBuf>,
    load: &mut Vec<String>,
) {
    for entry in entries {
        if entry.is_folder {
            egui::CollapsingHeader::new(&entry.name)
                .id_salt(&entry.path)
                .show(ui, |ui| {
                    if let Some(children) = &entry.children {
                        native_branch(ui, children, active, open, load);
                        if children.is_empty() {
                            ui.label("No supported files.");
                        }
                    } else if let Some(error) = &entry.error {
                        ui.label(error);
                        if ui.button("Retry").clicked() {
                            load.push(entry.path.clone());
                        }
                    } else {
                        ui.label("Loading…");
                        if !entry.loading {
                            load.push(entry.path.clone());
                        }
                    }
                });
        } else if ui
            .selectable_label(active == Some(entry.path.as_str()), &entry.name)
            .clicked()
        {
            *open = Some(PathBuf::from(&entry.path));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::Fixture;

    #[test]
    fn nested_file_open_and_save_preserve_tree_root() {
        let mut f = Fixture::new();
        let nested = f.dir.join("notes/deep");
        std::fs::create_dir_all(&nested).unwrap();
        let note = nested.join("note.md");
        std::fs::write(&note, "# Nested").unwrap();
        f.app.open_folder(f.dir.clone());
        let root = f.app.folder.clone();
        let branch = f.app.entries[0].path.clone();
        assert!(f.app.entries[0].children.is_none());
        f.request(serde_json::json!({"type":"loadFolder","path":branch}));
        let deep = &f.app.entries[0].children.as_ref().unwrap()[0];
        assert!(deep.children.is_none());
        let deep_path = deep.path.clone();
        f.app.load_branch(deep_path);
        assert_eq!(
            f.app.entries[0].children.as_ref().unwrap()[0]
                .children
                .as_ref()
                .unwrap()[0]
                .name,
            "note.md"
        );
        f.app.open_path(note.clone());
        assert_eq!(f.app.folder, root);
        assert_eq!(f.app.recents[0].path, storage::normalize_path(&note));
        f.app.run_command("save");
        assert_eq!(f.app.folder, root);
    }

    #[test]
    fn branches_ignore_stale_results_and_allow_retry_after_error() {
        let mut f = Fixture::new();
        std::fs::create_dir_all(f.dir.join("child")).unwrap();
        f.app.open_folder(f.dir.clone());
        let path = f.app.entries[0].path.clone();
        let generation = f.app.folder_generation;
        f.app
            .finish_branch(&path, generation - 1, Err("stale".into()));
        assert!(f.app.entries[0].error.is_none());
        f.app
            .finish_branch(&path, generation, Err("unavailable".into()));
        assert!(f.app.entries[0].error.is_some());
        f.app.load_branch(path);
        assert!(f.app.entries[0].children.as_ref().unwrap().is_empty());
        assert!(f.app.entries[0].error.is_none());
        // Arbitrary paths outside the visible tree cannot trigger a directory read.
        f.app.needs_sync = false;
        f.app.load_branch(f.dir.to_string_lossy().into());
        assert!(!f.app.needs_sync);
    }
}
