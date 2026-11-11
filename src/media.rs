//! Local media resolution and byte-range streaming for custom-protocol assets.
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::{Arc, RwLock},
};

const MAX_CHUNK: u64 = 1024 * 1024;

#[derive(Default)]
pub struct MediaRoots {
    pub docs: HashMap<u64, PathBuf>,
    pub pdf_files: HashMap<u64, PathBuf>,
    pub folder: Option<PathBuf>,
}

pub type SharedMediaRoots = Arc<RwLock<MediaRoots>>;

pub struct MediaResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub content_range: Option<String>,
    pub body: Vec<u8>,
}

pub fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg" | "oga") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("flac") => "audio/flac",
        Some("aac") => "audio/aac",
        Some("weba") => "audio/webm",
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("ogv") => "video/ogg",
        Some("mov") => "video/quicktime",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

pub fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16)?;
            let lo = (bytes[i + 2] as char).to_digit(16)?;
            out.push(((hi << 4) | lo) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

pub fn resolve_media_path(roots: &MediaRoots, uri: &str) -> Option<PathBuf> {
    let without_query = uri.split(['?', '#']).next().unwrap_or(uri);
    let path_part = without_query
        .strip_prefix("markora://localhost/")
        .or_else(|| without_query.strip_prefix("http://markora.localhost/"))
        .or_else(|| without_query.strip_prefix("https://markora.localhost/"))
        .or_else(|| without_query.strip_prefix('/'))?;
    if let Some(rest) = path_part.strip_prefix("pdf/") {
        let (id_str, _) = rest.split_once('/')?;
        let doc_id: u64 = id_str.parse().ok()?;
        let target = roots.pdf_files.get(&doc_id)?.canonicalize().ok()?;
        return target.is_file().then_some(target);
    }
    let rest = path_part.strip_prefix("doc/")?;
    let (id_str, encoded_rel) = rest.split_once('/')?;
    let doc_id: u64 = id_str.parse().ok()?;
    let rel = percent_decode(encoded_rel)?.replace('\\', "/");
    let mut clean = PathBuf::new();
    for comp in Path::new(&rel).components() {
        match comp {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let base = roots.docs.get(&doc_id).or(roots.folder.as_ref())?;
    let base_canon = base.canonicalize().ok()?;
    let target = base_canon.join(&clean).canonicalize().ok()?;
    if target.starts_with(&base_canon) && target.is_file() {
        Some(target)
    } else {
        None
    }
}

fn parse_range(header: &str, total: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (start_s, end_s) = spec.split_once('-')?;
    if total == 0 {
        return None;
    }
    if start_s.is_empty() {
        let suffix: u64 = end_s.parse().ok()?;
        let start = total.saturating_sub(suffix);
        Some((start, total - 1))
    } else {
        let start: u64 = start_s.parse().ok()?;
        if start >= total {
            return None;
        }
        let end = if end_s.is_empty() {
            (start + MAX_CHUNK - 1).min(total - 1)
        } else {
            end_s.parse::<u64>().ok()?.min(total - 1)
        };
        (start <= end).then_some((start, end))
    }
}

pub fn serve_media(roots: &MediaRoots, uri: &str, range_header: Option<&str>) -> MediaResponse {
    let Some(path) = resolve_media_path(roots, uri) else {
        return MediaResponse {
            status: 404,
            content_type: "text/plain",
            content_range: None,
            body: b"Not found".to_vec(),
        };
    };
    let Ok(mut file) = File::open(&path) else {
        return MediaResponse {
            status: 404,
            content_type: "text/plain",
            content_range: None,
            body: b"Unreadable".to_vec(),
        };
    };
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let content_type = mime_type(&path);
    if let Some(range) = range_header {
        let Some((start, end)) = parse_range(range, total) else {
            return MediaResponse {
                status: 416,
                content_type: "text/plain",
                content_range: Some(format!("bytes */{total}")),
                body: Vec::new(),
            };
        };
        let len = (end - start + 1) as usize;
        let mut body = vec![0u8; len];
        if file.seek(SeekFrom::Start(start)).is_ok() && file.read_exact(&mut body).is_ok() {
            return MediaResponse {
                status: 206,
                content_type,
                content_range: Some(format!("bytes {start}-{end}/{total}")),
                body,
            };
        }
    }
    let mut body = Vec::with_capacity(total as usize);
    let _ = file.read_to_end(&mut body);
    MediaResponse {
        status: 200,
        content_type,
        content_range: None,
        body,
    }
}
