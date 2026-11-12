//! Persist image-folder preferences and exact external files imported by this app.
use super::*;
use markora::{
    asset_policy::{AssetMode, AssetPolicy},
    image_assets::ImportedImage,
};

impl Markora {
    pub(super) fn asset_policy(&self) -> AssetPolicy {
        AssetPolicy {
            mode: self.prefs.asset_mode,
            custom_path: self.prefs.asset_custom_path.clone(),
        }
    }
    pub(super) fn initialize_image_registry(&mut self) {
        self.images.known_paths = self.storage.read("imported-images.json");
        if let Ok(mut roots) = self.media_roots.write() {
            roots.imported_images = self.images.known_paths.clone();
        }
    }
    pub(super) fn register_images(&mut self, images: &[ImportedImage]) -> Result<(), String> {
        if images.is_empty() {
            return Ok(());
        }
        for image in images {
            self.images.known_paths.insert(image.source_path.clone());
        }
        if let Ok(mut roots) = self.media_roots.write() {
            roots.imported_images = self.images.known_paths.clone();
        }
        self.storage
            .write("imported-images.json", &self.images.known_paths)
            .map_err(|e| e.to_string())
    }
    pub(super) fn pick_asset_folder(&mut self) {
        if let Some(folder) = rfd::FileDialog::new()
            .set_title("Choose a folder for imported images")
            .pick_folder()
        {
            self.prefs.asset_mode = AssetMode::Custom;
            self.prefs.asset_custom_path = folder.to_string_lossy().into();
            self.persist_prefs();
        }
    }

    pub(super) fn asset_settings_ui(&mut self, ctx: &egui::Context) {
        if !self.images.show_settings {
            return;
        }
        let mut open = true;
        let mut mode = self.prefs.asset_mode;
        let mut custom = self.prefs.asset_custom_path.clone();
        let mut browse = false;
        egui::Window::new("Image asset folders").open(&mut open).show(ctx, |ui| {
            egui::ComboBox::from_id_salt("asset-mode").selected_text(match mode { AssetMode::File => "file_name.assets", AssetMode::Shared => "data.assets", AssetMode::Custom => "Custom folder" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut mode, AssetMode::File, "Per file: file_name.assets");
                ui.selectable_value(&mut mode, AssetMode::Shared, "Shared: data.assets");
                ui.selectable_value(&mut mode, AssetMode::Custom, "Custom folder");
            });
            if mode == AssetMode::Custom {
                ui.text_edit_singleline(&mut custom);
                browse = ui.button("Browse…").clicked();
                ui.label("Relative to each note, or an absolute shared folder. Other drives use file URLs.");
            }
            ui.label("Used for new imports. Existing links are preserved.");
        });
        self.images.show_settings = open;
        if mode != self.prefs.asset_mode || custom != self.prefs.asset_custom_path {
            self.prefs.asset_mode = mode;
            self.prefs.asset_custom_path = custom;
            self.persist_prefs();
        }
        if browse {
            self.pick_asset_folder();
        }
    }
}
