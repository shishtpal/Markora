//! Application lifecycle. File operations, IPC, commands and fallback live in submodules.
mod asset_settings;
mod bridge;
mod commands;
mod conflict;
mod export;
mod fallback;
mod files;
#[cfg(test)]
mod image_tests;
mod images;
#[cfg(test)]
mod loading_tests;
mod pdf_state;
mod pdf_viewer;
mod reading;
#[cfg(test)]
mod reading_tests;
mod safety;
#[cfg(test)]
mod safety_tests;
mod save;
mod session;
mod sidebar;
#[cfg(test)]
mod tests;

use crate::{theme, webview_host::Host};
use eframe::egui;
use files::BgEvent;
use markora::{
    media::SharedMediaRoots,
    model::{DocKind, Prefs, PreparedDocument, Recent, Workspace},
    protocol::FileEntry,
    storage::{self, Storage},
};
use pdf_state::PdfViewerState;
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

pub type BridgeQueue = Arc<Mutex<VecDeque<String>>>;

#[derive(Clone)]
enum CloseAction {
    Tab(u64),
    All,
    Exit,
}
struct PendingClose {
    action: CloseAction,
    documents: VecDeque<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SyncedDocState {
    pub id: u64,
    pub kind: DocKind,
    pub revision: u64,
    pub view_mode: markora::model::ViewMode,
    pub rendered_revision: u64,
    pub rendered_full: bool,
}

pub struct Markora {
    workspace: Workspace,
    prefs: Prefs,
    recents: Vec<Recent>,
    storage: Storage,
    folder: Option<PathBuf>,
    entries: Vec<FileEntry>,
    status: String,
    error: bool,
    pending: Option<PendingClose>,
    allow_close: bool,
    needs_sync: bool,
    page_ready: bool,
    webview: Option<Host>,
    webview_attempted: bool,
    native_handle: Option<raw_window_handle::RawWindowHandle>,
    egui_ctx: Option<egui::Context>,
    queue: BridgeQueue,
    media_roots: SharedMediaRoots,
    bg_tx: mpsc::Sender<BgEvent>,
    bg_rx: mpsc::Receiver<BgEvent>,
    pending_opens: HashSet<PathBuf>,
    folder_generation: u64,
    pending_print: Option<(u64, u64, u64, Option<PathBuf>)>,
    print_token: u64,
    last_dark: Option<bool>,
    synced_doc: Option<SyncedDocState>,
    client_known_markdown: Option<(u64, u64)>,
    fallback_buf: Option<(u64, u64, String)>,
    pdf_viewer: PdfViewerState,
    reading: reading::ReadingState,
    safety: safety::SafetyState,
    images: images::ImageState,
}

impl Markora {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        use raw_window_handle::HasWindowHandle;
        let storage = Storage::discover();
        let prefs = storage.prefs();
        let recents = storage.read("recents.json");
        let initial_arg = std::env::args_os().nth(1).map(PathBuf::from);
        let initial_md = if initial_arg.is_some() {
            String::new()
        } else {
            include_str!("../assets/welcome.md").into()
        };
        let workspace = Workspace::new(initial_md, prefs.view_mode);
        let (bg_tx, bg_rx) = mpsc::channel();
        let dark = prefs.color_mode == "dark";
        theme::apply(&cc.egui_ctx, dark);
        let mut app = Self {
            workspace,
            prefs,
            recents,
            storage,
            folder: None,
            entries: Vec::new(),
            status: "Ready".into(),
            error: false,
            pending: None,
            allow_close: false,
            needs_sync: true,
            page_ready: false,
            webview: None,
            webview_attempted: false,
            native_handle: cc.window_handle().ok().map(|h| h.as_raw()),
            egui_ctx: Some(cc.egui_ctx.clone()),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            media_roots: SharedMediaRoots::default(),
            bg_tx,
            bg_rx,
            pending_opens: HashSet::new(),
            folder_generation: 0,
            pending_print: None,
            print_token: 0,
            last_dark: Some(dark),
            synced_doc: None,
            client_known_markdown: None,
            fallback_buf: None,
            pdf_viewer: PdfViewerState::default(),
            reading: reading::ReadingState::default(),
            safety: safety::SafetyState::default(),
            images: images::ImageState::default(),
        };
        if let Some(path) = initial_arg {
            app.open_path(path);
        }
        app.ensure_webview(&cc.egui_ctx);
        app.initialize_image_registry();
        app.initialize_session();
        app
    }

    fn notify(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.error = false;
        self.needs_sync = true;
    }
    fn fail(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.error = true;
        self.needs_sync = true;
    }
    fn persist_prefs(&mut self) {
        if let Err(error) = self.storage.write("prefs.json", &self.prefs) {
            self.fail(format!("Could not save settings: {error}"));
        }
        self.needs_sync = true;
    }

    fn ensure_webview(&mut self, ctx: &egui::Context) {
        if self.webview_attempted {
            return;
        }
        self.webview_attempted = true;
        match Host::create(
            self.native_handle,
            self.queue.clone(),
            ctx.clone(),
            self.media_roots.clone(),
            self.prefs.color_mode == "dark",
        ) {
            Ok(host) => self.webview = Some(host),
            Err(error) if cfg!(windows) => {
                self.fail(format!("{error}. Using native source editor."));
            }
            Err(_) => {}
        }
    }
}

impl eframe::App for Markora {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.ensure_webview(&ctx);
        self.drain_bridge();
        self.drain_bg();
        self.reading_frame(&ctx);
        self.pdf_viewer.drain(&ctx);
        let exports = self
            .webview
            .as_ref()
            .map(Host::take_exports)
            .unwrap_or_default();
        for outcome in exports {
            match outcome {
                Ok(path) => {
                    let message = format!("Exported {}", path.display());
                    if storage::is_pdf_path(&path) {
                        self.open_path(path);
                    }
                    self.notify(message);
                }
                Err(err) => self.fail(err),
            }
        }
        let dark = self.prefs.color_mode == "dark";
        if self.last_dark != Some(dark) {
            theme::apply(&ctx, dark);
            self.last_dark = Some(dark);
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if let Some(host) = &self.webview {
                if let Err(error) = host.send(&serde_json::json!({"type":"requestClose"})) {
                    self.fail(error);
                }
            } else {
                self.request_close(CloseAction::Exit);
            }
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if self.webview.is_none() && !dropped.is_empty() {
            self.native_drop(dropped, (0, 0));
        }

        if self.webview.is_some() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    let (_, rect) = ui.allocate_space(ui.available_size());
                    if let Some(host) = &mut self.webview {
                        let scale = ctx.pixels_per_point();
                        if let Err(error) = host.set_bounds(rect, scale) {
                            self.fail(error);
                        }
                    }
                });
        } else {
            self.fallback_ui(ui);
            self.conflict_ui(&ctx);
            self.asset_settings_ui(&ctx);
        }

        if !self.allow_close {
            self.scan_files(&ctx);
            self.checkpoint_session();
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        self.sync_webview();
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
