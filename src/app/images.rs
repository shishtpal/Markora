//! Host-side transfer lifecycle and revision-bound image jobs.
use super::*;
use markora::{
    image_assets::{self, ImageInput, ImportedImage},
    image_protocol::{ImageReply, ImageRequest, Transfer},
};

#[derive(Default)]
pub(super) struct ImageState {
    pub show_settings: bool,
    policy: Option<markora::asset_policy::AssetPolicy>,
    pub known_paths: std::collections::HashSet<PathBuf>,
    pub reply: Option<ImageReply>,
    transfer: Option<Transfer>,
    running: bool,
    pub native_range: std::ops::Range<usize>,
    native_insert: Option<std::ops::Range<usize>>,
}

impl Markora {
    pub(super) fn image_request(&mut self, request: ImageRequest) {
        self.needs_sync = true;
        let result = (|| -> Result<(), String> {
            match request {
                ImageRequest::Begin {
                    token,
                    doc_id,
                    revision,
                    files,
                } => {
                    let transfer = Transfer::new(files)?;
                    self.begin_images(token, doc_id, revision)?;
                    self.images.transfer = Some(transfer);
                }
                ImageRequest::Paths {
                    token,
                    doc_id,
                    revision,
                    paths,
                } => {
                    if paths.is_empty() || paths.len() > image_assets::MAX_FILES {
                        return Err("Choose up to 10 images.".into());
                    }
                    self.begin_images(token, doc_id, revision)?;
                    self.spawn_images(move || image_assets::read_inputs(&paths));
                }
                ImageRequest::Chunk {
                    token,
                    seq,
                    file_index,
                    data,
                    last,
                } => {
                    let reply = self
                        .images
                        .reply
                        .as_mut()
                        .ok_or("No active image transfer")?;
                    if reply.token != token || reply.status != "ready" {
                        return Err("This image transfer is no longer active.".into());
                    }
                    self.images
                        .transfer
                        .as_mut()
                        .ok_or("No image transfer")?
                        .chunk(reply, seq, file_index, &data, last)?;
                }
                ImageRequest::Finish { token } => {
                    let reply = self.images.reply.as_ref().ok_or("No image transfer")?;
                    if reply.token != token || reply.status != "ready" {
                        return Err("This image transfer is no longer active.".into());
                    }
                    let transfer = self.images.transfer.take().ok_or("No image transfer")?;
                    if reply.file_index != transfer.specs.len() {
                        return Err("Image transfer was incomplete.".into());
                    }
                    if !self.image_target_current() {
                        return Err("The document changed during the transfer. Paste or drop the images again.".into());
                    }
                    self.spawn_images(move || Ok(transfer.inputs));
                }
                ImageRequest::Dismiss { token } => {
                    if !self.images.running
                        && self.images.reply.as_ref().is_some_and(|r| r.token == token)
                    {
                        self.images.reply = None;
                        self.images.transfer = None;
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            if !self.images.running {
                self.images.transfer = None;
                if let Some(reply) = self.images.reply.as_mut() {
                    reply.status = "error".into();
                    reply.message = error.clone();
                }
            }
            self.fail(error);
        }
    }

    fn begin_images(&mut self, token: u64, id: u64, revision: u64) -> Result<(), String> {
        if self.images.running {
            return Err("An image import is already running.".into());
        }
        self.images.transfer = None;
        self.images.reply = Some(ImageReply {
            token,
            doc_id: id,
            revision,
            ..Default::default()
        });
        let doc = self.workspace.doc();
        if doc.id != id
            || doc.revision != revision
            || doc.kind != DocKind::Markdown
            || self.reading.active
            || self.pending.is_some()
        {
            return Err("Choose an editable Markdown document before inserting images.".into());
        }
        if doc.path.is_none() && !self.save_document(id, false) {
            return Err("Image import cancelled. Save the note before inserting images.".into());
        }
        self.images.reply.as_mut().unwrap().document_path = self
            .workspace
            .doc()
            .path
            .clone()
            .ok_or("Save the note first")?;
        self.images.reply.as_mut().unwrap().status = "ready".into();
        let policy = self.asset_policy();
        policy.folder(&self.images.reply.as_ref().unwrap().document_path)?;
        self.images.policy = Some(policy);
        self.notify("Importing images…");
        Ok(())
    }

    fn image_target_current(&self) -> bool {
        self.images.reply.as_ref().is_some_and(|r| {
            let doc = self.workspace.doc();
            doc.id == r.doc_id
                && doc.revision == r.revision
                && doc.path.as_ref() == Some(&r.document_path)
                && !self.reading.active
        })
    }

    fn spawn_images(
        &mut self,
        inputs: impl FnOnce() -> Result<Vec<ImageInput>, String> + Send + 'static,
    ) {
        let reply = self.images.reply.as_mut().unwrap();
        reply.status = "working".into();
        self.images.running = true;
        let token = reply.token;
        let path = reply.document_path.clone();
        let policy = self
            .images
            .policy
            .clone()
            .unwrap_or_else(|| self.asset_policy());
        let tx = self.bg_tx.clone();
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let result = inputs()
                .and_then(|inputs| image_assets::import_with_policy(&path, inputs, &policy));
            let _ = tx.send(BgEvent::ImagesImported { token, result });
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    pub(super) fn finish_images(&mut self, token: u64, result: Result<Vec<ImportedImage>, String>) {
        if self.images.reply.as_ref().is_none_or(|r| r.token != token) {
            return;
        }
        self.images.running = false;
        let registry_error = result
            .as_ref()
            .ok()
            .and_then(|images| self.register_images(images).err());
        let current = self.image_target_current();
        let reply = self.images.reply.as_mut().unwrap();
        match result {
            Ok(images) => {
                reply.images = images;
                reply.status = if current { "complete" } else { "stale" }.into();
                if self.webview.is_none()
                    && let Some(range) = self.images.native_insert.take()
                    && current
                {
                    let text = reply
                        .images
                        .iter()
                        .map(|i| {
                            format!(
                                "![Image]({})",
                                markora::asset_policy::encode_link(&i.relative_path)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let doc = self.workspace.doc_mut();
                    let mut chars: Vec<char> = doc.markdown.chars().collect();
                    let end = range.end.min(chars.len());
                    chars.splice(range.start.min(end)..end, text.chars());
                    doc.edit(chars.into_iter().collect(), doc.revision + 1);
                    self.fallback_buf = None;
                }
                self.notify(if current {
                    "Images imported"
                } else {
                    "Images imported; return to the original note to insert them."
                });
            }
            Err(error) => {
                reply.status = "error".into();
                reply.message = error.clone();
                self.fail(format!("Could not import images: {error}"));
            }
        }
        self.needs_sync = true;
        if let Some(error) = registry_error {
            self.fail(format!(
                "Images imported, but their registry could not be saved: {error}"
            ));
        }
    }

    pub(super) fn native_drop(&mut self, paths: Vec<PathBuf>, position: (i32, i32)) {
        if let Some(host) = &self.webview {
            if let Err(error) = host
                .send(&serde_json::json!({"type":"fileDrop", "paths":paths,"position":position}))
            {
                self.fail(error);
            }
        } else {
            let (images, documents): (Vec<_>, Vec<_>) = paths
                .into_iter()
                .partition(|p| image_assets::is_image_path(p));
            if !images.is_empty() {
                if !documents.is_empty() {
                    self.fail("Drop images and documents separately.");
                    return;
                }
                self.images.native_insert = Some(self.images.native_range.clone());
                let doc = self.workspace.doc();
                self.image_request(ImageRequest::Paths {
                    token: doc.revision + 1,
                    doc_id: doc.id,
                    revision: doc.revision,
                    paths: images,
                });
            } else {
                for path in documents {
                    self.open_path(path);
                }
            }
        }
    }
}
