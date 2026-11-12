//! Own OS file drops so browser and parent-window handlers cannot import them twice.
use crate::app::BridgeQueue;
use eframe::egui;

pub fn handler(queue: BridgeQueue, wake: egui::Context) -> impl Fn(wry::DragDropEvent) -> bool {
    move |event| {
        if let wry::DragDropEvent::Drop { paths, position } = event {
            if let Ok(mut messages) = queue.lock() {
                messages.push_back(
                    serde_json::json!({"type":"nativeDrop","paths":paths,"position":position})
                        .to_string(),
                );
            }
            wake.request_repaint();
        }
        true
    }
}
