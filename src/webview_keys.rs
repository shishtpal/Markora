//! WebView accelerators also cover focus inside Chromium's embedded PDF viewer.
use crate::app::BridgeQueue;
use eframe::egui;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use webview2_com::{
    AcceleratorKeyPressedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_PHYSICAL_KEY_STATUS,
    },
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
use wry::{WebView, WebViewExtWindows};

pub(super) fn install(
    webview: &WebView,
    queue: BridgeQueue,
    wake: egui::Context,
) -> Result<Arc<AtomicBool>, String> {
    let reading_pdf = Arc::new(AtomicBool::new(false));
    let intercept_escape = reading_pdf.clone();
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut key = 0;
        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
        let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
        // SAFETY: WebView2 provides valid event arguments for the callback duration.
        unsafe {
            args.VirtualKey(&mut key)?;
            args.KeyEventKind(&mut kind)?;
            args.PhysicalKeyStatus(&mut status)?;
        }
        let name = match key {
            0x7A => "F11",
            0x1B if intercept_escape.load(Ordering::Relaxed) => "Escape",
            _ => return Ok(()),
        };
        // Modified F11 belongs to other shortcuts. Handle repeats without toggling.
        if unsafe {
            [0x10, 0x11, 0x12, 0x5B, 0x5C]
                .iter()
                .any(|k| GetKeyState(*k) < 0)
        } {
            return Ok(());
        }
        unsafe {
            args.SetHandled(true)?;
        }
        if kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && !status.WasKeyDown.as_bool() {
            if let Ok(mut messages) = queue.lock() {
                messages.push_back(serde_json::json!({"type":"readingKey","key":name}).to_string());
            }
            // Defer script execution until after this synchronous COM callback returns.
            wake.request_repaint();
        }
        Ok(())
    }));
    let mut token = 0;
    unsafe {
        webview
            .controller()
            .add_AcceleratorKeyPressed(&handler, &mut token)
    }
    .map_err(|e| format!("Could not register reading shortcuts: {e}"))?;
    Ok(reading_pdf)
}
