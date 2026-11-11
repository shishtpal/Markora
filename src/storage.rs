//! Local settings and file I/O, independent of egui/WebView2.
use crate::{
    model::{Prefs, Recent},
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

pub fn write_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    use std::io::Write;
    // Truncate only after the destination has opened successfully; propagate all errors.
    let mut file = std::fs::File::create(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

pub fn folder_entries(dir: &Path) -> io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_folder = path.is_dir();
        if !is_folder
            && !path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown" | "txt")
            })
        {
            continue;
        }
        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            is_folder,
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
