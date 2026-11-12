//! User-selected image destinations and portable Markdown path construction.
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AssetMode {
    Shared,
    Custom,
    #[default]
    #[serde(other)]
    File,
}

#[derive(Clone, Debug, Default)]
pub struct AssetPolicy {
    pub mode: AssetMode,
    pub custom_path: String,
}

impl AssetPolicy {
    pub fn folder(&self, document: &Path) -> Result<PathBuf, String> {
        let parent = document
            .parent()
            .ok_or("Save the document before importing images.")?;
        match self.mode {
            AssetMode::File => Ok(parent.join(format!(
                "{}.assets",
                document
                    .file_stem()
                    .ok_or("Invalid note name")?
                    .to_string_lossy()
            ))),
            AssetMode::Shared => Ok(parent.join("data.assets")),
            AssetMode::Custom => {
                let raw = self.custom_path.trim();
                if raw.is_empty() || raw.chars().any(char::is_control) {
                    return Err(
                        "Set a custom image folder in Settings before importing images.".into(),
                    );
                }
                let path = PathBuf::from(raw);
                if path.is_absolute() {
                    Ok(path)
                } else if path
                    .components()
                    .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
                {
                    Err("Use a full absolute path or a path relative to the note.".into())
                } else {
                    Ok(parent.join(path))
                }
            }
        }
    }
}

pub fn link_path(base: &Path, target: &Path) -> Result<String, String> {
    let from: Vec<_> = base.components().collect();
    let to: Vec<_> = target.components().collect();
    if from.first() != to.first() {
        return url::Url::from_file_path(target)
            .map(String::from)
            .map_err(|_| "Could not construct an image file URL".into());
    }
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

pub fn encode_link(path: &str) -> String {
    if path.starts_with("file:") {
        return path.into();
    }
    let mut result = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            result.push(byte as char);
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

pub fn target(base: &Path, raw: &str) -> Option<PathBuf> {
    if raw.starts_with("file:") {
        return url::Url::parse(raw).ok()?.to_file_path().ok();
    }
    let decoded = crate::media::percent_decode(raw)?;
    let path = Path::new(&decoded);
    if path.is_absolute() || decoded.contains(':') {
        return None;
    }
    Some(base.join(decoded.replace('\\', "/")))
}
