//! Transient reading presentation; window placement is restored by winit on fullscreen exit.
use super::*;

#[derive(Default)]
pub(super) struct ReadingState {
    pub active: bool,
    restore_fullscreen: bool,
    expected_fullscreen: Option<bool>,
}

impl Markora {
    pub(super) fn forward_reading_key(&mut self, key: markora::protocol::ReadingKey) {
        if let Some(host) = &self.webview
            && let Err(error) = host.send(&serde_json::json!({"type":"readingKey","key":key}))
        {
            self.fail(error);
        }
    }

    pub(super) fn set_reading(&mut self, active: bool) {
        if self.reading.active == active {
            return;
        }
        if let Some(ctx) = &self.egui_ctx {
            if active && self.reading.expected_fullscreen.is_none() {
                self.reading.restore_fullscreen =
                    ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            }
            let fullscreen = active || self.reading.restore_fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
            self.reading.expected_fullscreen = Some(fullscreen);
        }
        self.reading.active = active;
        if active {
            self.workspace.doc_mut().ensure_rendered();
        }
        self.needs_sync = true;
    }

    pub(super) fn reading_frame(&mut self, ctx: &egui::Context) {
        if self.reading.expected_fullscreen.is_some()
            && self.reading.expected_fullscreen == ctx.input(|i| i.viewport().fullscreen)
        {
            self.reading.expected_fullscreen = None;
        }
        {
            let key = ctx.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                        ..
                    } if matches!(key, egui::Key::F11 | egui::Key::Escape) => Some(*key),
                    _ => None,
                })
            });
            if self.webview.is_some() {
                // The parent may own keyboard focus after a window activation.
                // Child accelerators mark their events handled, so these do not double-fire.
                if key == Some(egui::Key::F11) {
                    self.forward_reading_key(markora::protocol::ReadingKey::F11);
                } else if key == Some(egui::Key::Escape) && self.reading.active {
                    self.forward_reading_key(markora::protocol::ReadingKey::Escape);
                }
            } else if key == Some(egui::Key::F11) {
                self.set_reading(!self.reading.active);
            } else if key == Some(egui::Key::Escape)
                && self.pending.is_none()
                && self.safety.conflict.is_none()
            {
                self.set_reading(false);
            }
        }
        // File navigation and external reloads may replace a deferred Source document.
        if self.reading.active && !self.workspace.doc().rendered_full {
            self.workspace.doc_mut().ensure_rendered();
            self.needs_sync = true;
        }
        if let Some(host) = &self.webview {
            host.set_reading_pdf(self.reading.active && self.workspace.doc().kind == DocKind::Pdf);
        } else if self.reading.active {
            egui::Area::new(egui::Id::new("exit-reading"))
                .anchor(egui::Align2::RIGHT_TOP, [-12.0, 12.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if ui.button("Exit reading · F11").clicked() {
                        self.set_reading(false);
                    }
                });
        }
    }
}
