//! Typed IPC contract. All actions carry the latest editor draft.
use crate::model::{Prefs, ViewMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub doc_id: u64,
    pub markdown: String,
    pub revision: u64,
}

#[derive(Debug, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub draft: Option<Draft>,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Html,
    Pdf,
    Rtf,
    Txt,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HtmlStyle {
    #[default]
    Current,
    Light,
    Minimal,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Ready,
    Edit,
    Command { command: String },
    OpenPath { path: String },
    OpenLink { url: String },
    ActivateTab { id: u64 },
    CloseTab { id: u64 },
    Preferences { patch: PrefsPatch },
    ResolveUnsaved { choice: UnsavedChoice },
    #[serde(rename_all = "camelCase")]
    Export {
        format: ExportFormat,
        #[serde(default)]
        html_style: HtmlStyle,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PrefsPatch {
    pub color_mode: Option<String>,
    pub md_theme: Option<String>,
    pub view_mode: Option<ViewMode>,
    pub show_files: Option<bool>,
    pub show_outline: Option<bool>,
    pub focus_mode: Option<bool>,
    pub split_ratio: Option<f32>,
    pub font_scale: Option<f32>,
    pub files_width: Option<f32>,
    pub outline_width: Option<f32>,
}

impl PrefsPatch {
    pub fn apply(self, prefs: &mut Prefs) {
        macro_rules! assign { ($($field:ident),*) => { $(if let Some(v) = self.$field { prefs.$field = v; })* }; }
        assign!(
            color_mode,
            md_theme,
            view_mode,
            show_files,
            show_outline,
            focus_mode,
            split_ratio,
            font_scale,
            files_width,
            outline_width
        );
        prefs.validate();
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabView {
    pub id: u64,
    pub title: String,
    pub dirty: bool,
    pub path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_folder: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentView<'a> {
    pub id: u64,
    pub markdown: &'a str,
    pub revision: u64,
    pub view_mode: ViewMode,
    pub rendered: &'a crate::markdown::RenderedDocument,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot<'a> {
    pub tabs: Vec<TabView>,
    pub document: DocumentView<'a>,
    pub prefs: &'a Prefs,
    pub folder: Option<String>,
    pub entries: &'a [FileEntry],
    pub recents: &'a [crate::model::Recent],
    pub status: &'a str,
    pub error: bool,
    pub pending: Option<String>,
}
