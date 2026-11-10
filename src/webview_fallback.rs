//! Linux uses the native source editor without a WebKit/GTK dependency.
use crate::app::BridgeQueue;
use eframe::egui;

// No webview is constructed on this platform. Keeping the same interface lets
// the application share document, file and command handling with Windows.
pub struct Host;

impl Host {
    pub fn create(
        _handle: Option<raw_window_handle::RawWindowHandle>,
        _queue: BridgeQueue,
        _wake: egui::Context,
        _media_roots: markora::media::SharedMediaRoots,
    ) -> Result<Self, String> {
        Err("The web interface is currently available on Windows".into())
    }

    pub fn send(&self, _message: &impl serde::Serialize) -> Result<(), String> {
        Err("No webview is available".into())
    }

    pub fn print(&self) -> Result<(), String> {
        Err("Printing requires WebView2 on Windows".into())
    }

    pub fn export_pdf(&self, _path: &std::path::Path) -> Result<(), String> {
        Err("PDF export requires WebView2 on Windows".into())
    }

    pub fn take_exports(&self) -> Vec<Result<std::path::PathBuf, String>> {
        Vec::new()
    }

    pub fn set_bounds(&mut self, _rect: egui::Rect, _scale: f32) -> Result<(), String> {
        Ok(())
    }
}
