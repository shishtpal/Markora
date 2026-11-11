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

fn load_app_icon() -> Option<std::sync::Arc<eframe::egui::IconData>> {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/markora.png"))
        .ok()
        .map(std::sync::Arc::new)
}

fn main() -> eframe::Result {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([900.0, 560.0])
        .with_title("Markora — Markdown Editor");
    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Markora",
        options,
        Box::new(|cc| Ok(Box::new(app::Markora::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_app_icon_decodes() {
        let icon = super::load_app_icon().expect("assets/markora.png should decode");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}
