//! WebView2 lifetime and the transport only; no document or command logic.
use crate::app::BridgeQueue;
use eframe::egui;
use markora::media::{SharedMediaRoots, serve_media};
use std::{
    borrow::Cow,
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{ICoreWebView2_7, ICoreWebView2Environment6},
    PrintToPdfCompletedHandler,
};
use windows_core::{HSTRING, Interface, PCWSTR};
use wry::{WebView, WebViewBuilder, WebViewExtWindows};

pub type ExportQueue = Arc<Mutex<VecDeque<Result<PathBuf, String>>>>;

pub struct Host {
    webview: WebView,
    bounds: Option<wry::Rect>,
    exports: ExportQueue,
    wake: egui::Context,
}

impl Host {
    pub fn create(
        handle: Option<raw_window_handle::RawWindowHandle>,
        queue: BridgeQueue,
        wake: egui::Context,
        media_roots: SharedMediaRoots,
    ) -> Result<Self, String> {
        let Some(raw_window_handle::RawWindowHandle::Win32(handle)) = handle else {
            return Err("WebView2 requires a Windows window handle".into());
        };
        let parent = ParentWindow(handle.hwnd.get());
        let ipc_wake = wake.clone();
        let webview = WebViewBuilder::new()
            .with_html(load_html())
            .with_devtools(cfg!(debug_assertions))
            .with_asynchronous_custom_protocol("markora".into(), move |_id, request, responder| {
                let roots = media_roots.clone();
                std::thread::spawn(move || {
                    let uri = request.uri().to_string();
                    let range = request
                        .headers()
                        .get("range")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string);
                    let response = roots
                        .read()
                        .ok()
                        .map(|guard| serve_media(&guard, &uri, range.as_deref()))
                        .map(|m| {
                            let mut builder = wry::http::Response::builder()
                                .status(m.status)
                                .header("Content-Type", m.content_type)
                                .header("Accept-Ranges", "bytes")
                                .header("Access-Control-Allow-Origin", "*");
                            if let Some(cr) = m.content_range {
                                builder = builder.header("Content-Range", cr);
                            }
                            builder.body(Cow::Owned(m.body)).unwrap()
                        })
                        .unwrap_or_else(|| {
                            wry::http::Response::builder()
                                .status(500)
                                .body(Cow::Borrowed(&b""[..]))
                                .unwrap()
                        });
                    responder.respond(response);
                });
            })
            .with_ipc_handler(move |request| {
                if let Ok(mut messages) = queue.lock() {
                    messages.push_back(request.body().to_string());
                }
                // WebView clicks do not generate egui input. Every IPC must wake its event loop.
                ipc_wake.request_repaint();
            })
            // wry can load with_html through NavigateToString or a data URL.
            .with_navigation_handler(|url| {
                url == "about:blank" || url.starts_with("data:text/html;charset=utf-8;base64,")
            })
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .build_as_child(&parent)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            webview,
            bounds: None,
            exports: Arc::new(Mutex::new(VecDeque::new())),
            wake,
        })
    }

    pub fn send(&self, message: &impl serde::Serialize) -> Result<(), String> {
        let json = serde_json::to_string(message).map_err(|e| e.to_string())?;
        self.webview
            .evaluate_script(&format!("window.markoraReceive({json})"))
            .map_err(|e| e.to_string())
    }

    pub fn print(&self) -> Result<(), String> {
        self.webview.print().map_err(|e| e.to_string())
    }

    pub fn export_pdf(&self, path: &Path) -> Result<(), String> {
        let webview = self
            .webview
            .webview()
            .cast::<ICoreWebView2_7>()
            .map_err(|e| format!("PDF export requires WebView2 1.0.902+: {e}"))?;
        let settings = self
            .webview
            .environment()
            .cast::<ICoreWebView2Environment6>()
            .ok()
            .and_then(|env| unsafe { env.CreatePrintSettings() }.ok());
        if let Some(ref s) = settings {
            let _ = unsafe { s.SetShouldPrintBackgrounds(true) };
        }
        let target = path.to_path_buf();
        let wide = HSTRING::from(target.as_os_str());
        let exports = self.exports.clone();
        let wake = self.wake.clone();
        let handler = PrintToPdfCompletedHandler::create(Box::new(move |result, ok| {
            let outcome = match result {
                Ok(()) if ok => Ok(target),
                Ok(()) => Err("PDF generation did not succeed".into()),
                Err(err) => Err(format!("PDF export failed: {err}")),
            };
            if let Ok(mut queue) = exports.lock() {
                queue.push_back(outcome);
            }
            wake.request_repaint();
            Ok(())
        }));
        unsafe { webview.PrintToPdf(PCWSTR::from_raw(wide.as_ptr()), settings.as_ref(), &handler) }
            .map_err(|e| format!("Could not start PDF export: {e}"))
    }

    pub fn take_exports(&self) -> Vec<Result<PathBuf, String>> {
        self.exports
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn set_bounds(&mut self, rect: egui::Rect, scale: f32) -> Result<(), String> {
        let bounds = wry::Rect {
            position: wry::dpi::PhysicalPosition::new(
                (rect.min.x * scale) as i32,
                (rect.min.y * scale) as i32,
            )
            .into(),
            size: wry::dpi::PhysicalSize::new(
                (rect.width() * scale) as u32,
                (rect.height() * scale) as u32,
            )
            .into(),
        };
        if self.bounds.as_ref() != Some(&bounds) {
            self.webview.set_bounds(bounds).map_err(|e| e.to_string())?;
            self.bounds = Some(bounds);
        }
        Ok(())
    }
}

struct ParentWindow(isize);
impl wry::raw_window_handle::HasWindowHandle for ParentWindow {
    fn window_handle(
        &self,
    ) -> Result<wry::raw_window_handle::WindowHandle<'_>, wry::raw_window_handle::HandleError> {
        let hwnd = std::num::NonZeroIsize::new(self.0)
            .ok_or(wry::raw_window_handle::HandleError::Unavailable)?;
        let handle = wry::raw_window_handle::Win32WindowHandle::new(hwnd);
        // SAFETY: eframe owns the parent HWND for the lifetime of this child webview.
        Ok(unsafe {
            wry::raw_window_handle::WindowHandle::borrow_raw(
                wry::raw_window_handle::RawWindowHandle::Win32(handle),
            )
        })
    }
}

fn load_html() -> String {
    if cfg!(debug_assertions) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/ui/index.html");
        if let Ok(html) = std::fs::read_to_string(path) {
            return html;
        }
    }
    include_str!("../assets/ui/index.html").into()
}
