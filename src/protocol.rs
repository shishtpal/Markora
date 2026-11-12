//! Typed IPC contract. All actions carry the latest editor draft.
use crate::model::{DocKind, Prefs, ViewMode};
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
    PickAssetFolder,
    Image {
        request: crate::image_protocol::ImageRequest,
    },
    NativeDrop {
        paths: Vec<std::path::PathBuf>,
        position: (i32, i32),
    },
    Ready,
    Edit,
    ReadingKey {
        key: ReadingKey,
    },
    PrintReady {
        token: u64,
    },
    Command {
        command: String,
    },
    OpenPath {
        path: String,
    },
    #[serde(rename_all = "camelCase")]
    OpenRelative {
        doc_id: u64,
        path: String,
    },
    OpenLink {
        url: String,
    },
    ActivateTab {
        id: u64,
    },
    CloseTab {
        id: u64,
    },
    Preferences {
        patch: PrefsPatch,
    },
    ResolveUnsaved {
        choice: UnsavedChoice,
    },
    ResolveConflict {
        choice: ConflictChoice,
        token: u64,
    },
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

#[derive(Debug, Deserialize, Serialize)]
pub enum ReadingKey {
    F11,
    Escape,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictChoice {
    Reload,
    Keep,
    SaveAs,
    Cancel,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PrefsPatch {
    pub asset_mode: Option<crate::asset_policy::AssetMode>,
    pub asset_custom_path: Option<String>,
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
            asset_mode,
            asset_custom_path,
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
    pub kind: DocKind,
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
    pub kind: DocKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<&'a str>,
    pub revision: u64,
    pub view_mode: ViewMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered: Option<&'a crate::markdown::RenderedDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf_url: Option<String>,
    pub word_count: usize,
    pub char_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot<'a> {
    pub image_import: Option<&'a crate::image_protocol::ImageReply>,
    pub tabs: Vec<TabView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<DocumentView<'a>>,
    pub active_doc_id: u64,
    pub reading: bool,
    pub view_mode: ViewMode,
    pub word_count: usize,
    pub char_count: usize,
    pub prefs: &'a Prefs,
    pub folder: Option<String>,
    pub entries: &'a [FileEntry],
    pub recents: &'a [crate::model::Recent],
    pub status: &'a str,
    pub error: bool,
    pub pending: Option<String>,
    pub conflict: Option<ConflictView<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView<'a> {
    pub token: u64,
    pub doc_id: u64,
    pub path: &'a str,
    pub deleted: bool,
}
