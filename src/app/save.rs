//! File picking for saves, guarded replacement and clean-baseline updates.
use super::*;
use markora::{model::DocKind, storage};

impl Markora {
    pub(super) fn save_document(&mut self, id: u64, save_as: bool) -> bool {
        let Some(index) = self.workspace.tabs.iter().position(|d| d.id == id) else {
            return false;
        };
        let doc = &self.workspace.tabs[index];
        if doc.kind != DocKind::Markdown {
            self.notify("PDF documents are read-only");
            return false;
        }
        let path = if save_as || doc.path.is_none() {
            let mut dialog = rfd::FileDialog::new()
                .add_filter("Markdown", &["md", "markdown", "txt"])
                .set_file_name(
                    doc.path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled.md"),
                );
            if let Some(parent) = doc.path.as_ref().and_then(|p| p.parent()) {
                dialog = dialog.set_directory(parent);
            }
            let Some(path) = dialog.save_file() else {
                return false;
            };
            if path.extension().is_none() {
                path.with_extension("md")
            } else {
                path
            }
        } else {
            doc.path.clone().expect("checked path")
        };
        self.save_to_path(id, path)
    }

    pub(super) fn save_to_path(&mut self, id: u64, path: PathBuf) -> bool {
        let Some(index) = self
            .workspace
            .tabs
            .iter()
            .position(|d| d.id == id && d.kind == DocKind::Markdown)
        else {
            return false;
        };
        let path = storage::normalize_path(&path);
        if self
            .workspace
            .tabs
            .iter()
            .any(|d| d.id != id && d.path.as_ref() == Some(&path))
        {
            self.fail("That file is already open in another tab. Choose another name.");
            return false;
        }
        let doc = &self.workspace.tabs[index];
        let expected = if doc.path.as_ref() == Some(&path) {
            doc.disk_version
        } else {
            match markora::file_io::FileVersion::read(&path) {
                Ok(version) => version,
                Err(error) => {
                    self.fail(format!("Could not check save destination: {error}"));
                    return false;
                }
            }
        };
        let relocated = if let Some(previous) = &doc.path
            && previous != &path
        {
            markora::image_assets::relocate_references(
                &doc.markdown,
                previous,
                &path,
                &self.asset_policy(),
                &self.images.known_paths,
            )
        } else {
            Ok((doc.markdown.clone(), Vec::new()))
        };
        let (markdown, images) = match relocated {
            Ok(result) => result,
            Err(error) => {
                self.fail(format!(
                    "Could not copy imported images for Save As: {error}"
                ));
                return false;
            }
        };
        match markora::file_io::write_checked(&path, markdown.as_bytes(), Some(expected)) {
            Ok(()) => {
                let doc = &mut self.workspace.tabs[index];
                if doc.markdown != markdown {
                    doc.edit(markdown, doc.revision + 1);
                }
                doc.disk_version = markora::file_io::FileVersion::of(doc.markdown.as_bytes());
                doc.path = Some(path.clone());
                doc.mark_saved();
                let title = doc.title();
                if let Some(parent) = path.parent() {
                    self.open_folder(parent.into());
                }
                self.touch_recent(path.clone(), title);
                self.notify(format!("Saved {}", path.display()));
                if let Err(error) = self.register_images(&images) {
                    self.fail(format!(
                        "Saved the note, but its image registry could not be saved: {error}"
                    ));
                }
                true
            }
            Err(error) => {
                if error.kind() == std::io::ErrorKind::AlreadyExists
                    && self.workspace.tabs[index].path.as_ref() == Some(&path)
                    && let Ok(version) = markora::file_io::FileVersion::read(&path)
                {
                    self.show_conflict(id, version, true);
                    return false;
                }
                self.fail(format!("Could not save {}: {error}", path.display()));
                false
            }
        }
    }

    pub(super) fn touch_recent(&mut self, path: PathBuf, title: String) {
        storage::touch_recent(&mut self.recents, path, title);
        if let Err(error) = self.storage.write("recents.json", &self.recents) {
            self.fail(format!("Could not save recent files: {error}"));
        }
    }
}
