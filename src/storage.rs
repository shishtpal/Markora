//! Local settings and file I/O, independent of egui/WebView2.
use crate::{
    media::percent_decode,
    model::{Document, Prefs, PreparedDocument, Recent, ViewMode},
    protocol::FileEntry,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    io,
    path::{Path, PathBuf},
};

pub struct Storage {
    pub dir: PathBuf,
}

impl Storage {
    pub fn discover() -> Self {
        Self::discover_with(
            std::env::var_os("MARKORA_DATA_DIR").map(PathBuf::from),
            std::env::current_exe().ok(),
            std::env::var_os("APPDATA").map(PathBuf::from),
        )
    }

    pub fn discover_with(
        data_dir_override: Option<PathBuf>,
        exe_path: Option<PathBuf>,
        app_data: Option<PathBuf>,
    ) -> Self {
        if let Some(dir) = data_dir_override {
            return Self { dir };
        }
        if let Some(exe_dir) = exe_path
            .as_deref()
            .and_then(Path::parent)
            .filter(|p| !p.as_os_str().is_empty())
        {
            let portable = exe_dir.join(".markora");
            if std::fs::create_dir_all(&portable).is_ok() {
                return Self { dir: portable };
            }
        }
        Self {
            dir: app_data.unwrap_or_default().join("Markora"),
        }
    }
    pub fn read<T: DeserializeOwned + Default>(&self, name: &str) -> T {
        std::fs::read(self.dir.join(name))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
    pub fn write(&self, name: &str, value: &impl Serialize) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
        write_file(&self.dir.join(name), &bytes)
    }
    pub fn prefs(&self) -> Prefs {
        let mut prefs: Prefs = self.read("prefs.json");
        prefs.validate();
        prefs
    }
}

pub fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

pub fn is_supported_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "md" | "markdown" | "txt" | "pdf"
        )
    })
}

pub fn read_markdown(path: &Path) -> io::Result<String> {
    let mut text = std::fs::read_to_string(path)?;
    if text.starts_with('\u{feff}') {
        text.drain(..'\u{feff}'.len_utf8());
    }
    Ok(text)
}

/// Normalize existing files and new save destinations to the same tab identity.
pub fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        path.parent()
            .and_then(|p| p.canonicalize().ok())
            .zip(path.file_name())
            .map(|(parent, name)| parent.join(name))
            .unwrap_or_else(|| path.to_path_buf())
    })
}

pub use crate::file_io::write_file;

pub fn folder_entries(dir: &Path) -> io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_folder = path.is_dir();
        if !is_folder && !is_supported_path(&path) {
            continue;
        }
        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            is_folder,
            children: None,
            loading: false,
            error: None,
        });
    }
    entries.sort_by(|a, b| {
        b.is_folder
            .cmp(&a.is_folder)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn touch_recent(recents: &mut Vec<Recent>, path: PathBuf, title: String) {
    recents.retain(|r| r.path != path);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    recents.insert(0, Recent { path, title, ts });
    recents.truncate(12);
}

pub fn prepare_file(
    path: &Path,
    view_mode: ViewMode,
    parse_full_pdf: bool,
) -> Result<PreparedDocument, String> {
    if is_pdf_path(path) {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        Document::prepare_pdf(bytes, parse_full_pdf)
    } else {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let version = crate::file_io::FileVersion::of(&bytes);
        let markdown = String::from_utf8(bytes).map_err(|e| e.to_string())?;
        let mut prepared =
            Document::prepare(markdown.trim_start_matches('\u{feff}').into(), view_mode);
        prepared.disk_version = version;
        Ok(prepared)
    }
}

pub fn resolve_relative_link(
    base: Option<&Path>,
    raw_href: &str,
) -> Result<Option<PathBuf>, String> {
    let raw = raw_href.split(['?', '#']).next().unwrap_or(raw_href).trim();
    let lower = raw.to_ascii_lowercase();
    if raw.is_empty()
        || raw.chars().any(char::is_control)
        || lower.starts_with("javascript:")
        || lower.starts_with("data:")
    {
        return Ok(None);
    }
    let decoded =
        percent_decode(raw).ok_or_else(|| format!("Invalid relative link: {raw_href}"))?;
    let rel = PathBuf::from(decoded.replace('\\', "/"));
    let target = if rel.is_absolute() {
        rel
    } else if let Some(dir) = base {
        dir.join(rel)
    } else {
        return Err("Save the document or open a folder to follow relative file links.".into());
    };
    if !target.is_file() {
        return Err(format!("Linked file not found: {}", target.display()));
    }
    if !is_supported_path(&target) {
        return Err(format!("Unsupported file type: {}", target.display()));
    }
    Ok(Some(target))
}
