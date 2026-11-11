//! Native egui PDF viewer UI (toolbar, page scroll area, zoom, and search).
use super::pdf_state::{PdfRenderJob, PdfViewerState};
use eframe::egui;
use markora::model::Document;

impl PdfViewerState {
    pub(super) fn ui(&mut self, ui: &mut egui::Ui, doc: &Document) {
        let Some(pdf) = &doc.pdf else {
            ui.centered_and_justified(|ui| {
                ui.label("Loading PDF document…");
            });
            return;
        };
        let doc_id = doc.id;
        let revision = doc.revision;
        let page_count = pdf.page_count();
        self.tab_mut(doc_id, revision);

        let mut start_text_index = false;
        ui.horizontal_wrapped(|ui| {
            let tab = self.tabs.get_mut(&doc_id).expect("initialized");
            if ui
                .add_enabled(tab.current_page > 0, egui::Button::new("◀ Prev"))
                .clicked()
            {
                tab.current_page = tab.current_page.saturating_sub(1);
                tab.jump_to_page = Some(tab.current_page);
            }
            ui.label(format!("Page {} / {page_count}", tab.current_page + 1));
            if ui
                .add_enabled(
                    tab.current_page + 1 < page_count,
                    egui::Button::new("Next ▶"),
                )
                .clicked()
            {
                tab.current_page = (tab.current_page + 1).min(page_count - 1);
                tab.jump_to_page = Some(tab.current_page);
            }
            ui.separator();
            if ui.button("−").clicked() {
                tab.fit_width = false;
                tab.zoom = (tab.zoom / 1.15).clamp(0.25, 4.0);
            }
            ui.label(if tab.fit_width {
                "Fit".to_string()
            } else {
                format!("{}%", (tab.zoom * 100.0).round() as u32)
            });
            if ui.button("+").clicked() {
                tab.fit_width = false;
                tab.zoom = (tab.zoom * 1.15).clamp(0.25, 4.0);
            }
            if ui.selectable_label(tab.fit_width, "Fit width").clicked() {
                tab.fit_width = !tab.fit_width;
            }
            ui.separator();
            let search = ui.add(
                egui::TextEdit::singleline(&mut tab.search_query)
                    .hint_text("Search PDF…")
                    .desired_width(150.0),
            );
            if search.changed() {
                if tab.page_texts.is_none() && !tab.text_loading {
                    tab.text_loading = true;
                    start_text_index = true;
                }
                tab.update_matches();
                if let Some(&page) = tab.search_matches.first() {
                    tab.jump_to_page = Some(page);
                }
            }
            if !tab.search_query.trim().is_empty() {
                if tab.text_loading {
                    ui.label("Indexing…");
                } else if tab.search_matches.is_empty() {
                    ui.label("0 matches");
                } else {
                    ui.label(format!(
                        "{} / {} pages",
                        tab.active_match + 1,
                        tab.search_matches.len()
                    ));
                    if ui.small_button("◀").clicked() {
                        tab.active_match = (tab.active_match + tab.search_matches.len() - 1)
                            % tab.search_matches.len();
                        tab.jump_to_page = Some(tab.search_matches[tab.active_match]);
                    }
                    if ui.small_button("▶").clicked() {
                        tab.active_match = (tab.active_match + 1) % tab.search_matches.len();
                        tab.jump_to_page = Some(tab.search_matches[tab.active_match]);
                    }
                }
            }
        });
        ui.separator();

        if start_text_index {
            self.spawn_text_index(ui.ctx().clone(), doc_id, revision, pdf.clone());
        }

        let avail_w = (ui.available_width() - 24.0).max(160.0);
        let ppp = ui.ctx().pixels_per_point();
        let mut to_request = Vec::new();
        let mut center_page = None;
        let mut best_dist = f32::MAX;

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let clip = ui.clip_rect();
                let viewport_mid_y = clip.center().y;
                let (fit_width, zoom, jump) = {
                    let tab = self.tabs.get(&doc_id).expect("initialized");
                    (tab.fit_width, tab.zoom, tab.jump_to_page)
                };
                for (page_index, &(base_w, base_h)) in pdf.page_sizes().iter().enumerate() {
                    let logical_scale = if fit_width {
                        (avail_w / base_w.max(1.0)).clamp(0.25, 4.0)
                    } else {
                        zoom
                    };
                    let size = egui::vec2(base_w * logical_scale, base_h * logical_scale);
                    let render_scale = (logical_scale * ppp).clamp(0.25, 6.0);
                    let scale_key = (render_scale * 100.0).round() as u32;
                    ui.horizontal(|ui| {
                        let pad = ((ui.available_width() - size.x) * 0.5).max(8.0);
                        ui.add_space(pad);
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        if jump == Some(page_index) {
                            ui.scroll_to_rect(rect, Some(egui::Align::TOP));
                        }
                        let dist = (rect.center().y - viewport_mid_y).abs();
                        if dist < best_dist {
                            best_dist = dist;
                            center_page = Some(page_index);
                        }
                        let tab = self.tabs.get(&doc_id).expect("initialized");
                        let tex = tab.textures.get(&(page_index, scale_key)).or_else(|| {
                            tab.textures
                                .iter()
                                .find(|((p, _), _)| *p == page_index)
                                .map(|(_, h)| h)
                        });
                        if let Some(tex) = tex {
                            egui::Image::new(tex)
                                .fit_to_exact_size(size)
                                .paint_at(ui, rect);
                        } else {
                            ui.painter().rect_filled(
                                rect,
                                2.0,
                                egui::Color32::from_rgb(245, 246, 248),
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("Rendering page {}…", page_index + 1),
                                egui::FontId::proportional(14.0),
                                egui::Color32::from_gray(90),
                            );
                        }
                        if rect.intersects(clip.expand(450.0))
                            && !tab.textures.contains_key(&(page_index, scale_key))
                            && !tab.in_flight.contains(&(page_index, scale_key))
                        {
                            to_request.push((page_index, render_scale, scale_key));
                        }
                    });
                    ui.add_space(12.0);
                }
                if let Some(tab) = self.tabs.get_mut(&doc_id) {
                    tab.jump_to_page = None;
                    if let Some(page) = center_page {
                        tab.current_page = page;
                    }
                }
            });

        for (page_index, render_scale, scale_key) in to_request {
            self.spawn_render(
                ui.ctx().clone(),
                pdf.clone(),
                PdfRenderJob {
                    doc_id,
                    revision,
                    page_index,
                    render_scale,
                    scale_key,
                },
            );
        }
    }
}
