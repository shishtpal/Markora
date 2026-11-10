//! Native fallback styling. Web fonts belong to the HTML bundle only.
use eframe::egui::{self, Color32};

pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = if dark {
        Color32::from_rgb(23, 34, 49)
    } else {
        Color32::WHITE
    };
    visuals.selection.bg_fill = Color32::from_rgb(18, 70, 197);
    ctx.set_visuals(visuals);
}
