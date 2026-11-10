//! In-memory documents and validated preferences. No UI or filesystem calls.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::markdown::RenderedDocument;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Live,
    Source,
    Split,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    pub color_mode: String,
    pub md_theme: String,
    pub view_mode: ViewMode,
    pub show_files: bool,
    pub show_outline: bool,
    pub focus_mode: bool,
    pub split_ratio: f32,
    pub font_scale: f32,
    pub files_width: f32,
    pub outline_width: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            color_mode: "dark".into(),
            md_theme: "cobalt".into(),
            view_mode: ViewMode::Live,
            show_files: true,
            show_outline: true,
            focus_mode: false,
            split_ratio: 0.5,
            font_scale: 1.0,
            files_width: 240.0,
            outline_width: 220.0,
        }
    }
}

impl Prefs {
    pub fn validate(&mut self) {
        if !matches!(self.color_mode.as_str(), "light" | "dark") {
            self.color_mode = "dark".into();
        }
        if !Self::THEMES.contains(&self.md_theme.as_str()) {
            self.md_theme = "cobalt".into();
        }
        self.split_ratio = finite_clamp(self.split_ratio, 0.2, 0.8, 0.5);
        self.font_scale = finite_clamp(self.font_scale, 0.875, 1.5, 1.0);
        self.files_width = finite_clamp(self.files_width, 160.0, 560.0, 240.0);
        self.outline_width = finite_clamp(self.outline_width, 160.0, 480.0, 220.0);
    }

    pub const THEMES: [&'static str; 7] = [
        "cobalt", "paper", "graphite", "forest", "mono", "mars", "github",
    ];
}

fn finite_clamp(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Recent {
    pub path: PathBuf,
    pub title: String,
    pub ts: u64,
}

#[derive(Debug)]
pub struct PreparedDocument {
    pub markdown: String,
    pub rendered: RenderedDocument,
    pub rendered_full: bool,
    pub word_count: usize,
    pub char_count: usize,
}

#[derive(Debug)]
pub struct Document {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub markdown: String,
    pub saved_markdown: Option<String>,
    pub revision: u64,
    pub view_mode: ViewMode,
    pub rendered: RenderedDocument,
    pub rendered_full: bool,
    pub rendered_revision: u64,
    pub word_count: usize,
    pub char_count: usize,
}

const DEFER_SOURCE_RENDER_BYTES: usize = 64 * 1024;

impl Document {
    pub fn new(id: u64, markdown: String, view_mode: ViewMode) -> Self {
        Self::build(id, None, view_mode, Self::prepare(markdown, view_mode))
    }

    pub fn prepare(markdown: String, view_mode: ViewMode) -> PreparedDocument {
        let (word_count, char_count) = crate::markdown::text_counts(&markdown);
        let (rendered, rendered_full) = Self::render_for_mode(&markdown, view_mode);
        PreparedDocument {
            markdown,
            rendered,
            rendered_full,
            word_count,
            char_count,
        }
    }

    pub fn from_prepared(
        id: u64,
        path: PathBuf,
        view_mode: ViewMode,
        prepared: PreparedDocument,
    ) -> Self {
        Self::build(id, Some(path), view_mode, prepared)
    }

    fn build(
        id: u64,
        path: Option<PathBuf>,
        view_mode: ViewMode,
        prepared: PreparedDocument,
    ) -> Self {
        Self {
            id,
            path,
            saved_markdown: None,
            rendered: prepared.rendered,
            rendered_full: prepared.rendered_full,
            rendered_revision: 0,
            markdown: prepared.markdown,
            revision: 0,
            view_mode,
            word_count: prepared.word_count,
            char_count: prepared.char_count,
        }
    }

    pub fn render_for_mode(markdown: &str, view_mode: ViewMode) -> (RenderedDocument, bool) {
        if view_mode == ViewMode::Source && markdown.len() > DEFER_SOURCE_RENDER_BYTES {
            let outline = crate::markdown::render_outline(markdown);
            (
                RenderedDocument {
                    html: String::new(),
                    blocks: Vec::new(),
                    outline,
                },
                false,
            )
        } else {
            (crate::markdown::render(markdown), true)
        }
    }

    pub fn ensure_rendered(&mut self) {
        if !self.rendered_full {
            self.rendered = crate::markdown::render(&self.markdown);
            self.rendered_full = true;
            self.rendered_revision = self.revision;
        }
    }

    pub fn set_view_mode(&mut self, view_mode: ViewMode) {
        self.view_mode = view_mode;
        if view_mode != ViewMode::Source {
            self.ensure_rendered();
        }
    }

    pub fn edit(&mut self, markdown: String, revision: u64) {
        if revision < self.revision {
            return;
        }
        self.revision = revision;
        if self.markdown != markdown {
            if self.saved_markdown.is_none() {
                self.saved_markdown = Some(std::mem::take(&mut self.markdown));
            } else if self.saved_markdown.as_deref() == Some(markdown.as_str()) {
                self.saved_markdown = None;
            }
            let (word_count, char_count) = crate::markdown::text_counts(&markdown);
            let (rendered, rendered_full) = Self::render_for_mode(&markdown, self.view_mode);
            self.word_count = word_count;
            self.char_count = char_count;
            self.rendered = rendered;
            self.rendered_full = rendered_full;
            self.rendered_revision = revision;
            self.markdown = markdown;
        }
    }

    pub fn mark_saved(&mut self) {
        self.saved_markdown = None;
    }

    pub fn dirty(&self) -> bool {
        self.saved_markdown.is_some()
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                let title = self
                    .markdown
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches('#')
                    .trim();
                if title.is_empty() {
                    "Untitled".into()
                } else {
                    title.chars().take(60).collect()
                }
            })
    }
}

pub struct Workspace {
    pub tabs: Vec<Document>,
    pub active: usize,
    next_id: u64,
}

impl Workspace {
    pub fn new(markdown: String, view: ViewMode) -> Self {
        Self {
            tabs: vec![Document::new(1, markdown, view)],
            active: 0,
            next_id: 2,
        }
    }
    pub fn doc(&self) -> &Document {
        &self.tabs[self.active]
    }
    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.tabs[self.active]
    }
    pub fn add(&mut self, markdown: String, view: ViewMode) {
        let id = self.allocate_id();
        self.push_active(Document::new(id, markdown, view));
    }
    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn push_active(&mut self, doc: Document) {
        self.tabs.push(doc);
        self.active = self.tabs.len() - 1;
    }
    pub fn activate(&mut self, id: u64) {
        if let Some(index) = self.tabs.iter().position(|d| d.id == id) {
            self.active = index;
        }
    }
    pub fn close(&mut self, id: u64, default_view: ViewMode) {
        let Some(index) = self.tabs.iter().position(|d| d.id == id) else {
            return;
        };
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.add(String::new(), default_view);
        } else if index < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len() - 1);
        }
    }
    pub fn edit(&mut self, id: u64, markdown: String, revision: u64) {
        if let Some(doc) = self.tabs.iter_mut().find(|d| d.id == id) {
            doc.edit(markdown, revision);
        }
    }
}
