//! Bounded, acknowledged image transfers. Drafts accompany Begin, not each chunk.
use crate::image_assets::{CHUNK, ImageInput, ImportedImage, MAX_BATCH, MAX_FILES, MAX_IMAGE};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Instant};

#[derive(Debug, Deserialize)]
pub struct FileSpec {
    pub name: String,
    pub size: usize,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ImageRequest {
    #[serde(rename_all = "camelCase")]
    Begin {
        token: u64,
        doc_id: u64,
        revision: u64,
        files: Vec<FileSpec>,
    },
    #[serde(rename_all = "camelCase")]
    Paths {
        token: u64,
        doc_id: u64,
        revision: u64,
        paths: Vec<PathBuf>,
    },
    #[serde(rename_all = "camelCase")]
    Chunk {
        token: u64,
        seq: usize,
        file_index: usize,
        data: String,
        last: bool,
    },
    Finish {
        token: u64,
    },
    Dismiss {
        token: u64,
    },
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageReply {
    pub token: u64,
    pub doc_id: u64,
    pub revision: u64,
    pub document_path: PathBuf,
    pub status: String,
    pub message: String,
    pub next_seq: usize,
    pub file_index: usize,
    pub images: Vec<ImportedImage>,
}

pub struct Transfer {
    pub specs: Vec<FileSpec>,
    pub inputs: Vec<ImageInput>,
    pub started: Instant,
}
impl Transfer {
    pub fn new(specs: Vec<FileSpec>) -> Result<Self, String> {
        if specs.is_empty()
            || specs.len() > MAX_FILES
            || specs
                .iter()
                .any(|f| f.size == 0 || f.size > MAX_IMAGE || f.name.len() > 1024)
            || specs.iter().map(|f| f.size).sum::<usize>() > MAX_BATCH
        {
            return Err("Choose up to 10 images, at most 20 MiB each and 50 MiB together.".into());
        }
        let inputs = specs
            .iter()
            .map(|f| ImageInput {
                name: f.name.clone(),
                bytes: Vec::new(),
            })
            .collect();
        Ok(Self {
            specs,
            inputs,
            started: Instant::now(),
        })
    }
    pub fn chunk(
        &mut self,
        reply: &mut ImageReply,
        seq: usize,
        file_index: usize,
        data: &str,
        last: bool,
    ) -> Result<(), String> {
        if self.started.elapsed().as_secs() > 120
            || seq != reply.next_seq
            || file_index != reply.file_index
            || file_index >= self.specs.len()
            || data.len() > CHUNK.div_ceil(3) * 4
        {
            return Err("Image transfer expired or arrived out of order. Please retry.".into());
        }
        let bytes = STANDARD
            .decode(data)
            .map_err(|_| "Invalid image transfer bytes")?;
        let input = &mut self.inputs[file_index];
        if bytes.is_empty()
            || bytes.len() > CHUNK
            || input.bytes.len() + bytes.len() > self.specs[file_index].size
        {
            return Err("Image transfer exceeds its declared size.".into());
        }
        input.bytes.extend(bytes);
        if last {
            if input.bytes.len() != self.specs[file_index].size {
                return Err("Image transfer was incomplete.".into());
            }
            reply.file_index += 1;
        }
        reply.next_seq += 1;
        Ok(())
    }
}
