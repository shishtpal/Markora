//! Application lifecycle. File operations, IPC, commands and fallback live in submodules.
mod bridge;
mod commands;
mod fallback;
mod files;
#[cfg(test)]
mod tests;

use crate::{theme, webview_host::Host};
use eframe::egui;
use markora::{
    media::SharedMediaRoots,
    model::{Prefs, Recent, Workspace},
    protocol::FileEntry,
    storage::Storage,
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
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
    queue: BridgeQueue,
    media_roots: SharedMediaRoots,
}

impl Markora {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        use raw_window_handle::HasWindowHandle;
        let storage = Storage::discover();
        let prefs = storage.prefs();
        let recents = storage.read("recents.json");
        let workspace =
            Workspace::new(include_str!("../assets/welcome.md").into(), prefs.view_mode);
        Self {
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
            queue: Arc::new(Mutex::new(VecDeque::new())),
            media_roots: SharedMediaRoots::default(),
        }
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
        ) {
            Ok(host) => self.webview = Some(host),
            Err(error) if cfg!(windows) => {
                self.fail(format!("{error}. Using native source editor."));
            }
            Err(_) => self.notify("Ready"),
        }
    }
}

impl eframe::App for Markora {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.ensure_webview(&ctx);
        self.drain_bridge();
        let exports = self
            .webview
            .as_ref()
            .map(Host::take_exports)
            .unwrap_or_default();
        for outcome in exports {
            match outcome {
                Ok(path) => self.notify(format!("Exported {}", path.display())),
                Err(err) => self.fail(err),
            }
        }
        theme::apply(&ctx, self.prefs.color_mode == "dark");

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
        for path in dropped {
            self.open_path(path);
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
        }

        self.sync_webview();
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
