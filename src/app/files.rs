//! File dialogs and storage operations; documents remain in the workspace.
use super::*;
use markora::{
    model::{DocKind, Document, ViewMode},
    storage,
};
use std::path::Path;

pub(super) enum BgEvent {
    ImagesImported {
        token: u64,
        result: Result<Vec<markora::image_assets::ImportedImage>, String>,
    },
    DiskScan(Vec<safety::Observation>),
    FileLoaded {
        path: PathBuf,
        view_mode: markora::model::ViewMode,
        result: Result<PreparedDocument, String>,
    },
    FolderLoaded {
        path: PathBuf,
        generation: u64,
        result: Result<Vec<FileEntry>, String>,
    },
    BranchLoaded {
        path: String,
        generation: u64,
        result: Result<Vec<FileEntry>, String>,
    },
}

impl Markora {
    pub(super) fn drain_bg(&mut self) {
        let events: Vec<_> = self.bg_rx.try_iter().collect();
        for event in events {
            match event {
                BgEvent::ImagesImported { token, result } => self.finish_images(token, result),
                BgEvent::DiskScan(observations) => self.apply_observations(observations),
                BgEvent::BranchLoaded {
                    path,
                    generation,
                    result,
                } => {
                    self.finish_branch(&path, generation, result);
                }
                BgEvent::FileLoaded {
                    path,
                    view_mode,
                    result,
                } => {
                    self.pending_opens.remove(&path);
                    match result {
                        Ok(prepared) => self.finish_open_file(path, view_mode, prepared),
                        Err(error) => {
                            self.fail(format!("Could not open {}: {error}", path.display()));
                        }
                    }
                }
                BgEvent::FolderLoaded {
                    path,
                    generation,
                    result,
                } => {
                    if generation != self.folder_generation {
                        continue;
                    }
                    match result {
                        Ok(entries) => {
                            self.entries = entries;
                            self.notify(format!("Opened folder {}", path.display()));
                            self.folder = Some(path);
                        }
                        Err(error) => {
                            self.fail(format!("Could not read folder {}: {error}", path.display()));
                        }
                    }
                }
            }
        }
    }

    fn finish_open_file(&mut self, path: PathBuf, view_mode: ViewMode, prepared: PreparedDocument) {
        if let Some(doc) = self
            .workspace
            .tabs
            .iter_mut()
            .find(|d| d.path.as_ref() == Some(&path))
        {
            if prepared.kind == DocKind::Pdf {
                doc.reload_pdf(prepared);
            }
            let id = doc.id;
            self.workspace.activate(id);
            self.needs_sync = true;
            return;
        }
        let replace_empty = self.workspace.tabs.len() == 1
            && self.workspace.tabs[0].kind == DocKind::Markdown
            && self.workspace.tabs[0].path.is_none()
            && self.workspace.tabs[0].revision == 0
            && self.workspace.tabs[0].markdown.is_empty();
        let id = self.workspace.allocate_id();
        if replace_empty {
            self.workspace.tabs[0] = Document::from_prepared(id, path.clone(), view_mode, prepared);
            self.workspace.active = 0;
            self.synced_doc = None;
            self.fallback_buf = None;
        } else {
            self.workspace.push_active(Document::from_prepared(
                id,
                path.clone(),
                view_mode,
                prepared,
            ));
        }
        if let Some(parent) = path.parent()
            && self
                .folder
                .as_ref()
                .is_none_or(|root| !parent.starts_with(root))
        {
            self.open_folder(parent.into());
        }
        self.touch_recent(path.clone(), self.workspace.doc().title());
        self.notify(format!("Opened {}", path.display()));
    }

    pub(super) fn open_path(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.open_folder(path);
            return;
        }
        let path = storage::normalize_path(&path);
        let is_pdf = storage::is_pdf_path(&path);
        if !is_pdf
            && let Some(doc) = self
                .workspace
                .tabs
                .iter()
                .find(|d| d.path.as_ref() == Some(&path))
        {
            let id = doc.id;
            self.workspace.activate(id);
            self.needs_sync = true;
            return;
        }
        if self.pending_opens.contains(&path) {
            return;
        }
        let view_mode = self.prefs.view_mode;
        let parse_full_pdf = self.webview.is_none();
        if let Some(ctx) = self.egui_ctx.clone() {
            self.pending_opens.insert(path.clone());
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();
            self.notify(format!("Opening {name}…"));
            let tx = self.bg_tx.clone();
            std::thread::spawn(move || {
                let result = storage::prepare_file(&path, view_mode, parse_full_pdf);
                let _ = tx.send(BgEvent::FileLoaded {
                    path,
                    view_mode,
                    result,
                });
                ctx.request_repaint();
            });
            return;
        }
        match storage::prepare_file(&path, view_mode, parse_full_pdf) {
            Ok(prepared) => self.finish_open_file(path, view_mode, prepared),
            Err(error) => self.fail(format!("Could not open {}: {error}", path.display())),
        }
    }

    pub(super) fn open_relative(&mut self, doc_id: u64, raw_href: &str) {
        let base = self
            .workspace
            .tabs
            .iter()
            .find(|d| d.id == doc_id)
            .and_then(|d| d.path.as_deref())
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| self.folder.clone());
        match storage::resolve_relative_link(base.as_deref(), raw_href) {
            Ok(Some(target)) => self.open_path(target),
            Ok(None) => {}
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn open_folder(&mut self, path: PathBuf) {
        let path = storage::normalize_path(&path);
        self.folder_generation += 1;
        let generation = self.folder_generation;
        if let Some(ctx) = self.egui_ctx.clone() {
            let tx = self.bg_tx.clone();
            std::thread::spawn(move || {
                let result = storage::folder_entries(&path).map_err(|e| e.to_string());
                let _ = tx.send(BgEvent::FolderLoaded {
                    path,
                    generation,
                    result,
                });
                ctx.request_repaint();
            });
            return;
        }
        match storage::folder_entries(&path) {
            Ok(entries) => {
                self.entries = entries;
                self.notify(format!("Opened folder {}", path.display()));
                self.folder = Some(path);
            }
            Err(error) => self.fail(format!("Could not read folder {}: {error}", path.display())),
        }
    }

    pub(super) fn pick_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Supported Files", &["md", "markdown", "txt", "pdf"])
            .add_filter("Markdown / Text", &["md", "markdown", "txt"])
            .add_filter("PDF Documents", &["pdf"])
            .pick_file()
        {
            self.open_path(path);
        }
    }
    pub(super) fn pick_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.open_folder(path);
        }
    }
}
