//! Pure-Rust PDF parsing, page rasterization, and text extraction via hayro.
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, font::GlyphRun, hayro_cmap::BfString,
    interpret_page, util::TransformExt,
};
use hayro::hayro_syntax::{LoadPdfError, Pdf};
use hayro::kurbo::{BezPath, Rect};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use std::sync::Arc;

#[derive(Clone)]
pub struct PdfDocument {
    inner: Arc<Pdf>,
    page_sizes: Vec<(f32, f32)>,
}

impl std::fmt::Debug for PdfDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfDocument")
            .field("pages", &self.page_sizes.len())
            .finish()
    }
}

pub struct RenderedPdfPage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Quick header check used when the host WebView renders the PDF directly.
pub fn validate_header(bytes: &[u8]) -> Result<(), String> {
    let prefix_len = bytes.len().min(1024);
    if bytes[..prefix_len].windows(5).any(|w| w == b"%PDF-") {
        Ok(())
    } else {
        Err("File is not a valid PDF document.".into())
    }
}

impl PdfDocument {
    pub fn parse(bytes: Arc<Vec<u8>>) -> Result<Self, String> {
        let pdf = Pdf::new(bytes).map_err(|err| match err {
            LoadPdfError::Decryption(_) => {
                "Encrypted or password-protected PDFs are not supported.".to_string()
            }
            LoadPdfError::Invalid => "Could not parse PDF document.".to_string(),
        })?;
        let page_sizes: Vec<(f32, f32)> = pdf
            .pages()
            .iter()
            .map(|page| page.render_dimensions())
            .collect();
        if page_sizes.is_empty() {
            return Err("The PDF document contains no pages.".into());
        }
        Ok(Self {
            inner: Arc::new(pdf),
            page_sizes,
        })
    }

    pub fn page_count(&self) -> usize {
        self.page_sizes.len()
    }

    pub fn page_sizes(&self) -> &[(f32, f32)] {
        &self.page_sizes
    }

    pub fn render_page(&self, page_index: usize, scale: f32) -> Result<RenderedPdfPage, String> {
        let page = self
            .inner
            .pages()
            .get(page_index)
            .ok_or_else(|| format!("Page {} is out of range", page_index + 1))?;
        let (base_w, base_h) = self.page_sizes[page_index];
        let max_dim = base_w.max(base_h).max(1.0);
        let safe_scale = scale.clamp(0.2, 6.0).min(4096.0 / max_dim);
        let cache = RenderCache::new();
        let interpreter_settings = InterpreterSettings::default();
        let render_settings = RenderSettings::default();
        let pixmap_settings = PixmapSettings {
            x_scale: safe_scale,
            y_scale: safe_scale,
            bg_color: WHITE,
        };
        let pixmap = hayro::render(
            page,
            &cache,
            &interpreter_settings,
            &render_settings,
            &pixmap_settings,
        );
        let width = pixmap.width() as usize;
        let height = pixmap.height() as usize;
        let rgba = pixmap.take_rgba8(ImageAlphaType::Alpha);
        Ok(RenderedPdfPage {
            width,
            height,
            rgba,
        })
    }

    pub fn extract_page_texts(&self) -> Vec<String> {
        let cache = InterpreterCache::new();
        let settings = InterpreterSettings::default();
        self.inner
            .pages()
            .iter()
            .map(|page| {
                let (w, h) = page.render_dimensions();
                let target = Rect::new(0.0, 0.0, w as f64, h as f64);
                let transform = page.initial_transform(true).to_kurbo();
                let mut ctx =
                    Context::new(transform, target, &cache, page.xref(), settings.clone());
                let mut collector = TextCollector::default();
                interpret_page(page, &mut ctx, &mut collector);
                collector.text
            })
            .collect()
    }
}

#[derive(Default)]
struct TextCollector {
    text: String,
    last_pos: Option<(f64, f64)>,
}

impl<'a> Device<'a> for TextCollector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph_run(&mut self, glyph_run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, _: &DrawMode) {
        for glyph in glyph_run.glyphs() {
            let Some(unicode) = glyph.as_unicode() else {
                continue;
            };
            let xf = props.transform * glyph.transform();
            let pos = xf.translation();
            let c = xf.as_coeffs();
            let font_size = ((c[0].hypot(c[1]) + c[2].hypot(c[3])) * 500.0).max(6.0);
            if let Some((lx, ly)) = self.last_pos {
                if (pos.y - ly).abs() > font_size * 0.5 {
                    if !self.text.ends_with('\n') && !self.text.is_empty() {
                        self.text.push('\n');
                    }
                } else if pos.x - lx > font_size * 1.15 && !self.text.ends_with([' ', '\n']) {
                    self.text.push(' ');
                }
            }
            match unicode {
                BfString::Char(ch) => self.text.push(ch),
                BfString::String(s) => self.text.push_str(&s),
            }
            self.last_pos = Some((pos.x, pos.y));
        }
    }
}
