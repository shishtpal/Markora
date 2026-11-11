//! Background page rasterization, text indexing, and texture cache for native PDF viewing.
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use markora::pdf::PdfDocument;
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc,
};

enum PdfBgEvent {
    PageRendered {
        doc_id: u64,
        revision: u64,
        page_index: usize,
        scale_key: u32,
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
    PageFailed {
        doc_id: u64,
        revision: u64,
        page_index: usize,
        scale_key: u32,
    },
    TextsExtracted {
        doc_id: u64,
        revision: u64,
        texts: Vec<String>,
    },
}

pub(super) struct PdfTabState {
    pub(super) revision: u64,
    pub(super) zoom: f32,
    pub(super) fit_width: bool,
    pub(super) current_page: usize,
    pub(super) jump_to_page: Option<usize>,
    pub(super) textures: HashMap<(usize, u32), TextureHandle>,
    pub(super) in_flight: HashSet<(usize, u32)>,
    pub(super) page_texts: Option<Vec<String>>,
    pub(super) text_loading: bool,
    pub(super) search_query: String,
    pub(super) search_matches: Vec<usize>,
    pub(super) active_match: usize,
}

impl PdfTabState {
    pub(super) fn new(revision: u64) -> Self {
        Self {
            revision,
            zoom: 1.0,
            fit_width: true,
            current_page: 0,
            jump_to_page: None,
            textures: HashMap::new(),
            in_flight: HashSet::new(),
            page_texts: None,
            text_loading: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            active_match: 0,
        }
    }

    pub(super) fn update_matches(&mut self) {
        let q = self.search_query.trim().to_ascii_lowercase();
        self.search_matches.clear();
        self.active_match = 0;
        if q.is_empty() {
            return;
        }
        if let Some(texts) = &self.page_texts {
            for (idx, text) in texts.iter().enumerate() {
                if text.to_ascii_lowercase().contains(&q) {
                    self.search_matches.push(idx);
                }
            }
        }
    }
}

pub(super) struct PdfRenderJob {
    pub(super) doc_id: u64,
    pub(super) revision: u64,
    pub(super) page_index: usize,
    pub(super) render_scale: f32,
    pub(super) scale_key: u32,
}

pub(super) struct PdfViewerState {
    pub(super) tabs: HashMap<u64, PdfTabState>,
    tx: mpsc::Sender<PdfBgEvent>,
    rx: mpsc::Receiver<PdfBgEvent>,
}

impl Default for PdfViewerState {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tabs: HashMap::new(),
            tx,
            rx,
        }
    }
}

impl PdfViewerState {
    pub(super) fn tab_mut(&mut self, doc_id: u64, revision: u64) -> &mut PdfTabState {
        let entry = self
            .tabs
            .entry(doc_id)
            .or_insert_with(|| PdfTabState::new(revision));
        if entry.revision != revision {
            *entry = PdfTabState::new(revision);
        }
        entry
    }

    pub(super) fn remove_tab(&mut self, doc_id: u64) {
        self.tabs.remove(&doc_id);
    }

    pub(super) fn clear_all(&mut self) {
        self.tabs.clear();
    }

    pub(super) fn jump_to_page(&mut self, doc_id: u64, page_index: usize) {
        if let Some(tab) = self.tabs.get_mut(&doc_id) {
            tab.current_page = page_index;
            tab.jump_to_page = Some(page_index);
        }
    }

    pub(super) fn step_page(&mut self, doc_id: u64, delta: isize, page_count: usize) {
        if page_count == 0 {
            return;
        }
        let tab = self
            .tabs
            .entry(doc_id)
            .or_insert_with(|| PdfTabState::new(0));
        let next = (tab.current_page as isize + delta).clamp(0, page_count as isize - 1) as usize;
        tab.current_page = next;
        tab.jump_to_page = Some(next);
    }

    pub(super) fn adjust_zoom(&mut self, doc_id: u64, factor: Option<f32>) {
        let tab = self
            .tabs
            .entry(doc_id)
            .or_insert_with(|| PdfTabState::new(0));
        match factor {
            Some(mult) => {
                tab.fit_width = false;
                tab.zoom = (tab.zoom * mult).clamp(0.25, 4.0);
            }
            None => {
                tab.fit_width = false;
                tab.zoom = 1.0;
            }
        }
    }

    pub(super) fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                PdfBgEvent::PageRendered {
                    doc_id,
                    revision,
                    page_index,
                    scale_key,
                    width,
                    height,
                    rgba,
                } => {
                    let Some(tab) = self.tabs.get_mut(&doc_id) else {
                        continue;
                    };
                    if tab.revision != revision {
                        continue;
                    }
                    tab.in_flight.remove(&(page_index, scale_key));
                    tab.textures.retain(|&(p, _), _| p != page_index);
                    let image = ColorImage::from_rgba_unmultiplied([width, height], &rgba);
                    let handle = ctx.load_texture(
                        format!("pdf-{doc_id}-{revision}-{page_index}-{scale_key}"),
                        image,
                        TextureOptions::LINEAR,
                    );
                    tab.textures.insert((page_index, scale_key), handle);
                }
                PdfBgEvent::PageFailed {
                    doc_id,
                    revision,
                    page_index,
                    scale_key,
                } => {
                    if let Some(tab) = self.tabs.get_mut(&doc_id)
                        && tab.revision == revision
                    {
                        tab.in_flight.remove(&(page_index, scale_key));
                    }
                }
                PdfBgEvent::TextsExtracted {
                    doc_id,
                    revision,
                    texts,
                } => {
                    if let Some(tab) = self.tabs.get_mut(&doc_id)
                        && tab.revision == revision
                    {
                        tab.text_loading = false;
                        tab.page_texts = Some(texts);
                        tab.update_matches();
                    }
                }
            }
        }
    }

    pub(super) fn spawn_text_index(
        &self,
        ctx: egui::Context,
        doc_id: u64,
        revision: u64,
        pdf: PdfDocument,
    ) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let texts = pdf.extract_page_texts();
            let _ = tx.send(PdfBgEvent::TextsExtracted {
                doc_id,
                revision,
                texts,
            });
            ctx.request_repaint();
        });
    }

    pub(super) fn spawn_render(&mut self, ctx: egui::Context, pdf: PdfDocument, job: PdfRenderJob) {
        if let Some(tab) = self.tabs.get_mut(&job.doc_id) {
            tab.in_flight.insert((job.page_index, job.scale_key));
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let msg = match pdf.render_page(job.page_index, job.render_scale) {
                Ok(rendered) => PdfBgEvent::PageRendered {
                    doc_id: job.doc_id,
                    revision: job.revision,
                    page_index: job.page_index,
                    scale_key: job.scale_key,
                    width: rendered.width,
                    height: rendered.height,
                    rgba: rendered.rgba,
                },
                Err(_) => PdfBgEvent::PageFailed {
                    doc_id: job.doc_id,
                    revision: job.revision,
                    page_index: job.page_index,
                    scale_key: job.scale_key,
                },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }
}
