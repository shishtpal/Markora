//! Test the production dispatcher and close state machine without creating a window.
use super::*;
use markora::{model::ViewMode, protocol::UnsavedChoice};

pub(super) struct Fixture {
    pub(super) app: Markora,
    pub(super) dir: PathBuf,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/host-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let (bg_tx, bg_rx) = mpsc::channel();
        let app = Markora {
            workspace: Workspace::new("first".into(), ViewMode::Live),
            prefs: Prefs::default(),
            recents: Vec::new(),
            storage: Storage { dir: dir.clone() },
            folder: None,
            entries: Vec::new(),
            status: String::new(),
            error: false,
            pending: None,
            allow_close: false,
            needs_sync: true,
            page_ready: false,
            webview: None,
            webview_attempted: true,
            native_handle: None,
            egui_ctx: None,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            media_roots: Default::default(),
            bg_tx,
            bg_rx,
            pending_opens: HashSet::new(),
            folder_generation: 0,
            pending_print: None,
            print_token: 0,
            last_dark: None,
            synced_doc: None,
            client_known_markdown: None,
            fallback_buf: None,
            pdf_viewer: PdfViewerState::default(),
            reading: reading::ReadingState::default(),
            safety: safety::SafetyState::default(),
            images: images::ImageState::default(),
        };
        Self { app, dir }
    }
    pub(super) fn request(&mut self, message: serde_json::Value) {
        self.app
            .queue
            .lock()
            .unwrap()
            .push_back(message.to_string());
        self.app.drain_bridge();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.app.safety.writer.take();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn save_command_applies_its_draft_before_writing() {
    let mut fixture = Fixture::new();
    let path = fixture.dir.join("saved.md");
    fixture.app.workspace.doc_mut().path = Some(path.clone());
    fixture.request(
        serde_json::json!({"type":"command","command":"save","draft":{
            "docId":1,"markdown":"# नमस्ते 😀","revision":5
        }}),
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "# नमस्ते 😀");
    assert!(!fixture.app.workspace.doc().dirty());
}

#[test]
fn closing_inactive_tab_checks_that_document() {
    let mut fixture = Fixture::new();
    fixture
        .app
        .workspace
        .doc_mut()
        .edit("dirty first".into(), 1);
    fixture.app.workspace.add("second".into(), ViewMode::Source);
    fixture.request(serde_json::json!({"type":"closeTab","id":1}));
    assert_eq!(
        fixture.app.pending.as_ref().unwrap().documents.front(),
        Some(&1)
    );
    fixture.app.resolve_unsaved(UnsavedChoice::Discard);
    assert_eq!(fixture.app.workspace.doc().markdown, "second");
    assert!(fixture.app.pending.is_none());
}

#[test]
fn close_all_saves_each_dirty_document_by_id() {
    let mut fixture = Fixture::new();
    let first = fixture.dir.join("first.md");
    fixture.app.workspace.doc_mut().path = Some(first.clone());
    fixture
        .app
        .workspace
        .doc_mut()
        .edit("first changed".into(), 1);
    fixture.app.workspace.add("second".into(), ViewMode::Source);
    let second = fixture.dir.join("second.md");
    fixture.app.workspace.doc_mut().path = Some(second.clone());
    fixture
        .app
        .workspace
        .doc_mut()
        .edit("second changed".into(), 1);
    fixture.app.run_command("close-all");
    assert_eq!(fixture.app.pending.as_ref().unwrap().documents.len(), 2);
    fixture.app.resolve_unsaved(UnsavedChoice::Save);
    assert_eq!(std::fs::read_to_string(first).unwrap(), "first changed");
    fixture.app.resolve_unsaved(UnsavedChoice::Save);
    assert_eq!(std::fs::read_to_string(second).unwrap(), "second changed");
    assert!(fixture.app.pending.is_none());
    assert_eq!(fixture.app.workspace.tabs.len(), 1);
    assert_eq!(fixture.app.workspace.doc().markdown, "");
}

#[test]
fn malformed_commands_surface_an_error_and_preserve_documents() {
    let mut fixture = Fixture::new();
    fixture.request(serde_json::json!({"type":"unknown"}));
    assert!(fixture.app.error);
    assert_eq!(fixture.app.workspace.doc().markdown, "first");
}

#[test]
fn native_theme_command_flips_once_per_request() {
    let mut fixture = Fixture::new();
    fixture.request(serde_json::json!({"type":"command","command":"toggle-theme"}));
    assert_eq!(fixture.app.prefs.color_mode, "light");
    fixture.request(serde_json::json!({"type":"command","command":"toggle-theme"}));
    assert_eq!(fixture.app.prefs.color_mode, "dark");
}

#[test]
fn reopening_a_saved_file_activates_its_existing_tab() {
    let mut fixture = Fixture::new();
    let path = fixture.dir.join("saved.md");
    fixture.app.workspace.doc_mut().path = Some(path.clone());
    assert!(fixture.app.save_document(1, false));
    fixture.app.workspace.add("other".into(), ViewMode::Live);
    fixture
        .app
        .open_path(fixture.dir.join(".").join("saved.md"));
    assert_eq!(fixture.app.workspace.tabs.len(), 2);
    assert_eq!(fixture.app.workspace.doc().id, 1);
}

#[test]
fn export_to_path_writes_html_rtf_and_txt() {
    let mut fixture = Fixture::new();
    fixture
        .app
        .workspace
        .doc_mut()
        .edit("# Export Test\n\nHello **Markora**.\n".into(), 1);
    let html_path = fixture.dir.join("out.html");
    let rtf_path = fixture.dir.join("out.rtf");
    let txt_path = fixture.dir.join("out.txt");
    fixture.app.export_to_path(
        html_path.clone(),
        markora::protocol::ExportFormat::Html,
        markora::protocol::HtmlStyle::Light,
    );
    fixture.app.export_to_path(
        rtf_path.clone(),
        markora::protocol::ExportFormat::Rtf,
        markora::protocol::HtmlStyle::Current,
    );
    fixture.app.export_to_path(
        txt_path.clone(),
        markora::protocol::ExportFormat::Txt,
        markora::protocol::HtmlStyle::Current,
    );
    assert!(
        std::fs::read_to_string(html_path)
            .unwrap()
            .contains("<strong>Markora</strong>")
    );
    assert!(
        std::fs::read_to_string(rtf_path)
            .unwrap()
            .starts_with("{\\rtf1")
    );
    assert!(
        std::fs::read_to_string(txt_path)
            .unwrap()
            .contains("Hello Markora.")
    );
}

#[test]
fn source_mode_export_html_ensures_deferred_document_is_rendered() {
    let mut fixture = Fixture::new();
    let big_md = "# Big Heading\n\nBody paragraph.\n\n".repeat(3000);
    fixture.app.run_command("view-source");
    fixture.app.workspace.doc_mut().edit(big_md, 1);
    assert!(!fixture.app.workspace.doc().rendered_full);
    let html_path = fixture.dir.join("big.html");
    fixture.app.export_to_path(
        html_path.clone(),
        markora::protocol::ExportFormat::Html,
        markora::protocol::HtmlStyle::Minimal,
    );
    assert!(fixture.app.workspace.doc().rendered_full);
    assert!(
        std::fs::read_to_string(html_path)
            .unwrap()
            .contains("<h1>Big Heading</h1>")
    );
}

#[test]
fn pdf_tabs_open_stay_readonly_and_reload_on_reopen() {
    let mut fixture = Fixture::new();
    let md_path = fixture.dir.join("notes.md");
    let pdf_path = fixture.dir.join("guide.pdf");
    std::fs::write(&md_path, "[Guide](guide.pdf)").unwrap();
    let objs = [
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offs = Vec::new();
    for o in objs {
        offs.push(pdf.len());
        pdf.push_str(o);
    }
    let xref = pdf.len();
    pdf.push_str("xref\n0 4\n0000000000 65535 f \n");
    for off in offs {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    std::fs::write(&pdf_path, pdf).unwrap();

    fixture.app.open_path(md_path);
    let md_id = fixture.app.workspace.doc().id;
    fixture.request(serde_json::json!({
        "type": "openRelative",
        "docId": md_id,
        "path": "guide.pdf#page=1"
    }));
    let pdf_doc = fixture.app.workspace.doc();
    assert_eq!(pdf_doc.kind, DocKind::Pdf, "status: {}", fixture.app.status);
    assert!(!pdf_doc.dirty());
    let pdf_id = pdf_doc.id;

    // Edits and save commands must not dirty or overwrite a PDF tab.
    fixture.request(serde_json::json!({
        "type": "edit",
        "draft": { "docId": pdf_id, "markdown": "corrupt", "revision": 5 }
    }));
    fixture.app.run_command("save");
    assert_eq!(fixture.app.workspace.doc().markdown, "");
    assert!(!fixture.app.workspace.doc().dirty());
    assert_eq!(fixture.app.workspace.doc().revision, 0);

    // Re-opening the same PDF (e.g. after export) increments its revision to refresh the viewer.
    fixture.app.open_path(pdf_path);
    assert_eq!(fixture.app.workspace.doc().id, pdf_id);
    assert_eq!(fixture.app.workspace.doc().revision, 1);
}
