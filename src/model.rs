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
pub struct Document {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub markdown: String,
    pub saved_markdown: String,
    pub revision: u64,
    pub view_mode: ViewMode,
    pub rendered: RenderedDocument,
}

impl Document {
    pub fn new(id: u64, markdown: String, view_mode: ViewMode) -> Self {
        Self {
            id,
            path: None,
            saved_markdown: markdown.clone(),
            rendered: crate::markdown::render(&markdown),
            markdown,
            revision: 0,
            view_mode,
        }
    }

    pub fn edit(&mut self, markdown: String, revision: u64) {
        if revision < self.revision {
            return;
        }
        self.revision = revision;
        if self.markdown != markdown {
            self.rendered = crate::markdown::render(&markdown);
            self.markdown = markdown;
        }
    }

    pub fn dirty(&self) -> bool {
        self.markdown != self.saved_markdown
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
        self.tabs.push(Document::new(self.next_id, markdown, view));
        self.next_id += 1;
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
