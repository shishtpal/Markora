#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod platform;
mod theme;
#[cfg(not(windows))]
#[path = "webview_fallback.rs"]
mod webview_host;
#[cfg(windows)]
mod webview_host;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("Markora — Markdown Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "Markora",
        options,
        Box::new(|cc| Ok(Box::new(app::Markora::new(cc)))),
    )
}
