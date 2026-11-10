//! Export dialogs and file output for HTML, PDF, RTF, and plain text.
use super::*;
use markora::{
    export,
    protocol::{ExportFormat, HtmlStyle},
    storage,
};

impl Markora {
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
        if matches!(format, ExportFormat::Html) {
            self.workspace.doc_mut().ensure_rendered();
        }
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
                self.prepare_print(Some(path));
            }
        }
    }

    pub(super) fn prepare_print(&mut self, path: Option<PathBuf>) {
        if self.webview.is_none() {
            self.fail("Printing and PDF export require WebView2 on Windows");
            return;
        }
        self.workspace.doc_mut().ensure_rendered();
        self.needs_sync = true;
        self.sync_webview();
        let doc = self.workspace.doc();
        self.print_token += 1;
        let token = self.print_token;
        self.pending_print = Some((token, doc.id, doc.revision, path));
        let message = serde_json::json!({
            "type": "preparePrint", "token": token, "docId": doc.id, "revision": doc.revision
        });
        if let Err(error) = self.webview.as_ref().expect("checked host").send(&message) {
            self.pending_print = None;
            self.fail(error);
        }
    }

    pub(super) fn finish_print(&mut self, token: u64) {
        if self.pending_print.as_ref().is_none_or(|p| p.0 != token) {
            return;
        }
        let (_, id, revision, path) = self.pending_print.take().expect("checked token");
        let doc = self.workspace.doc();
        if doc.id != id || doc.revision != revision {
            self.fail("The document changed while preparing to print. Please try again.");
            return;
        }
        let Some(host) = &self.webview else { return };
        let result = match path {
            Some(path) => host.export_pdf(&path),
            None => host.print(),
        };
        match result {
            Ok(()) => self.notify("Printing document…"),
            Err(error) => self.fail(error),
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
