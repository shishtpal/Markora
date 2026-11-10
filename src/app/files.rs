//! File dialogs and storage operations; documents remain in the workspace.
use super::*;
use markora::{
    export,
    protocol::{ExportFormat, HtmlStyle},
    storage,
};

impl Markora {
    pub(super) fn open_path(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.open_folder(path);
            return;
        }
        let path = storage::normalize_path(&path);
        if let Some(doc) = self
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
        match storage::read_markdown(&path) {
            Ok(markdown) => {
                self.workspace.add(markdown, self.prefs.view_mode);
                self.workspace.doc_mut().path = Some(path.clone());
                if let Some(parent) = path.parent() {
                    self.open_folder(parent.into());
                }
                self.touch_recent(path.clone(), self.workspace.doc().title());
                self.notify(format!("Opened {}", path.display()));
            }
            Err(error) => self.fail(format!("Could not open {}: {error}", path.display())),
        }
    }

    pub(super) fn open_folder(&mut self, path: PathBuf) {
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
            .add_filter("Markdown / Text", &["md", "markdown", "txt"])
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

    pub(super) fn save_document(&mut self, id: u64, save_as: bool) -> bool {
        let Some(index) = self.workspace.tabs.iter().position(|d| d.id == id) else {
            return false;
        };
        let doc = &self.workspace.tabs[index];
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
        match storage::write_file(&path, self.workspace.tabs[index].markdown.as_bytes()) {
            Ok(()) => {
                let doc = &mut self.workspace.tabs[index];
                doc.path = Some(path.clone());
                doc.saved_markdown = doc.markdown.clone();
                let title = doc.title();
                if let Some(parent) = path.parent() {
                    self.open_folder(parent.into());
                }
                self.touch_recent(path.clone(), title);
                self.notify(format!("Saved {}", path.display()));
                true
            }
            Err(error) => {
                self.fail(format!("Could not save {}: {error}", path.display()));
                false
            }
        }
    }

    fn touch_recent(&mut self, path: PathBuf, title: String) {
        storage::touch_recent(&mut self.recents, path, title);
        if let Err(error) = self.storage.write("recents.json", &self.recents) {
            self.fail(format!("Could not save recent files: {error}"));
        }
    }

    pub(super) fn export_document(&mut self, format: ExportFormat, html_style: HtmlStyle) {
        let doc = self.workspace.doc();
        let (label, ext) = match format {
            ExportFormat::Html => ("HTML Document", "html"),
            ExportFormat::Pdf => ("PDF Document", "pdf"),
            ExportFormat::Rtf => ("Rich Text Format", "rtf"),
            ExportFormat::Txt => ("Plain Text", "txt"),
        };
        let stem = doc
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .map(String::from)
            .unwrap_or_else(|| sanitize_stem(&doc.title()));
        let mut dialog = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("{stem}.{ext}"));
        if let Some(parent) = doc.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(parent);
        }
        let Some(mut path) = dialog.save_file() else {
            return;
        };
        if path.extension().is_none() {
            path = path.with_extension(ext);
        }
        self.export_to_path(path, format, html_style);
    }

    pub(super) fn export_to_path(
        &mut self,
        path: PathBuf,
        format: ExportFormat,
        html_style: HtmlStyle,
    ) {
        let doc = self.workspace.doc();
        let title = doc.title();
        match format {
            ExportFormat::Html => {
                let html = export::to_html(
                    &title,
                    &doc.rendered.html,
                    &self.prefs.color_mode,
                    &self.prefs.md_theme,
                    html_style,
                );
                self.write_export(&path, html.as_bytes());
            }
            ExportFormat::Rtf => {
                let rtf = export::to_rtf(&title, &doc.markdown);
                self.write_export(&path, rtf.as_bytes());
            }
            ExportFormat::Txt => {
                let txt = export::to_plain_text(&doc.markdown);
                self.write_export(&path, txt.as_bytes());
            }
            ExportFormat::Pdf => {
                if let Some(host) = &self.webview {
                    match host.export_pdf(&path) {
                        Ok(()) => self.notify("Exporting PDF…"),
                        Err(error) => self.fail(error),
                    }
                } else {
                    self.fail("PDF export requires WebView2 on Windows");
                }
            }
        }
    }

    fn write_export(&mut self, path: &std::path::Path, bytes: &[u8]) {
        match storage::write_file(path, bytes) {
            Ok(()) => self.notify(format!("Exported {}", path.display())),
            Err(error) => self.fail(format!("Could not export {}: {error}", path.display())),
        }
    }
}

fn sanitize_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "untitled".into()
    } else {
        trimmed.into()
    }
}
