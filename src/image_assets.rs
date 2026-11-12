//! Bounded image validation and immutable, document-relative asset storage.
mod relocate;
use image::{ImageFormat, ImageReader, Limits};
pub use relocate::copy_references;
pub use relocate::relocate_references;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_IMAGE: usize = 20 * 1024 * 1024;
pub const MAX_BATCH: usize = 50 * 1024 * 1024;
pub const MAX_FILES: usize = 10;
pub const CHUNK: usize = 256 * 1024;

pub struct ImageInput {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedImage {
    pub source_path: PathBuf,
    pub relative_path: String,
    pub alt: String,
}

pub fn is_image_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "svg"
                | "bmp"
                | "ico"
                | "avif"
                | "tif"
                | "tiff"
        )
    })
}

pub fn read_inputs(paths: &[PathBuf]) -> Result<Vec<ImageInput>, String> {
    if paths.is_empty() || paths.len() > MAX_FILES {
        return Err("Choose between 1 and 10 images.".into());
    }
    let mut total = 0;
    paths
        .iter()
        .map(|path| {
            let file = fs::File::open(path).map_err(|e| e.to_string())?;
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("Only image files can be imported.".into());
            }
            let mut bytes = Vec::new();
            file.take((MAX_IMAGE + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            total += bytes.len();
            if bytes.len() > MAX_IMAGE || total > MAX_BATCH {
                return Err("Images must be at most 20 MiB each and 50 MiB together.".into());
            }
            Ok(ImageInput {
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                bytes,
            })
        })
        .collect()
}

fn validate(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE {
        return Err("Image must be between 1 byte and 20 MiB.".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "The file is not a supported image.")?;
    let extension = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        _ => return Err("Supported image formats are PNG, JPEG, GIF and WebP.".into()),
    };
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 32_000_000 {
        return Err("Image dimensions must not exceed 32 megapixels.".into());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(32_000);
    limits.max_image_height = Some(32_000);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|e| format!("The image could not be decoded: {e}"))?;
    Ok(extension)
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn contained_dir(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let folder = root.join(relative);
    // Check existing ancestors before creating anything through a symlink/reparse point.
    let mut existing = folder.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or("Invalid assets directory")?;
    }
    if !existing
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(root)
    {
        return Err("The assets folder points outside the document directory.".into());
    }
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let canonical = folder.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root) {
        return Err("The assets folder is outside the document directory.".into());
    }
    Ok(canonical)
}

fn store(root: &Path, relative: &Path, bytes: &[u8]) -> Result<(), String> {
    let folder = contained_dir(root, relative.parent().ok_or("Invalid image path")?)?;
    let target = folder.join(relative.file_name().ok_or("Invalid image name")?);
    if target.symlink_metadata().is_ok() {
        let canonical = target.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(root) {
            return Err("Image destination points outside the document directory.".into());
        }
        let existing = read_inputs(std::slice::from_ref(&target))?.remove(0);
        return if existing.bytes == bytes {
            Ok(())
        } else {
            Err("An image with this name already exists with different contents.".into())
        };
    }
    let mut temp = tempfile::Builder::new()
        .prefix(".import-")
        .tempfile_in(&folder)
        .map_err(|e| e.to_string())?;
    temp.write_all(bytes)
        .and_then(|()| temp.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    // Use the platform's no-overwrite publication, including Windows volumes without hard links.
    let published = temp.persist_noclobber(&target);
    match published {
        Ok(_) => {
            #[cfg(unix)]
            fs::File::open(&folder)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            store(root, relative, bytes)
        }
        Err(e) => Err(format!("Could not publish the image: {e}")),
    }
}

pub fn import(document: &Path, inputs: Vec<ImageInput>) -> Result<Vec<ImportedImage>, String> {
    import_with_policy(
        document,
        inputs,
        &crate::asset_policy::AssetPolicy::default(),
    )
}

pub fn import_with_policy(
    document: &Path,
    inputs: Vec<ImageInput>,
    policy: &crate::asset_policy::AssetPolicy,
) -> Result<Vec<ImportedImage>, String> {
    if inputs.is_empty()
        || inputs.len() > MAX_FILES
        || inputs.iter().map(|i| i.bytes.len()).sum::<usize>() > MAX_BATCH
    {
        return Err("Import up to 10 images, at most 50 MiB together.".into());
    }
    // Validate the entire batch before creating its assets directory.
    let extensions = inputs
        .iter()
        .map(|i| validate(&i.bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let root = document
        .parent()
        .ok_or("Save the document before importing images.")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let selected = policy.folder(&root.join(document.file_name().ok_or("Invalid note name")?))?;
    if policy.mode != crate::asset_policy::AssetMode::Custom && !selected.exists() {
        contained_dir(
            &root,
            selected
                .strip_prefix(&root)
                .map_err(|_| "Invalid image folder")?,
        )?;
    }
    fs::create_dir_all(&selected).map_err(|e| e.to_string())?;
    let folder = selected.canonicalize().map_err(|e| e.to_string())?;
    if policy.mode != crate::asset_policy::AssetMode::Custom && !folder.starts_with(&root) {
        return Err("The assets folder points outside the note directory.".into());
    }
    inputs
        .into_iter()
        .zip(extensions)
        .map(|(input, extension)| {
            let filename = format!("image-{}.{}", hash(&input.bytes), extension);
            store(&folder, Path::new(&filename), &input.bytes)?;
            let source_path = folder.join(&filename);
            let relative_path = crate::asset_policy::link_path(&root, &source_path)?;
            let alt = Path::new(&input.name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect::<String>();
            Ok(ImportedImage {
                source_path,
                relative_path,
                alt: if alt.is_empty() { "Image".into() } else { alt },
            })
        })
        .collect()
}
