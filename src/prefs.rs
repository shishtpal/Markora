//! Validated preferences and recent-file records.
use crate::model::ViewMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    pub asset_mode: crate::asset_policy::AssetMode,
    pub asset_custom_path: String,
    pub color_mode: String,
    pub md_theme: String,
    pub view_mode: ViewMode,
    pub show_files: bool,
    pub show_outline: bool,
    pub focus_mode: bool,
    pub split_ratio: f32,
    pub font_scale: f32,
    pub font_family: String,
    pub font_size: f32,
    pub files_width: f32,
    pub outline_width: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            asset_mode: Default::default(),
            asset_custom_path: String::new(),
            color_mode: "dark".into(),
            md_theme: "cobalt".into(),
            view_mode: ViewMode::Live,
            show_files: true,
            show_outline: true,
            focus_mode: false,
            split_ratio: 0.5,
            font_scale: 1.0,
            font_family: "theme".into(),
            font_size: 16.0,
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
        if !Self::FONT_FAMILIES.contains(&self.font_family.as_str()) {
            self.font_family = "theme".into();
        }
        self.font_size = finite_clamp(self.font_size, 12.0, 32.0, 16.0);
        self.files_width = finite_clamp(self.files_width, 160.0, 560.0, 240.0);
        self.outline_width = finite_clamp(self.outline_width, 160.0, 480.0, 220.0);
    }

    pub const THEMES: [&'static str; 7] = [
        "cobalt", "paper", "graphite", "forest", "mono", "mars", "github",
    ];
    pub const FONT_FAMILIES: [&'static str; 7] = [
        "theme",
        "space-grotesk",
        "system",
        "georgia",
        "arial",
        "ibm-plex-mono",
        "consolas",
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
