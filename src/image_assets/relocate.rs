//! Copy only referenced content-addressed Markora images during cross-directory Save As.
use super::*;
use pulldown_cmark::{Event, Parser, Tag};
use std::collections::HashSet;

fn managed_filename(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some((name, ext)) = name.rsplit_once('.') else {
        return false;
    };
    name.strip_prefix("image-")
        .is_some_and(|digest| digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
        && matches!(ext, "png" | "jpg" | "gif" | "webp")
}

pub fn relocate_references(
    markdown: &str,
    from: &Path,
    to: &Path,
    policy: &crate::asset_policy::AssetPolicy,
    known: &HashSet<PathBuf>,
) -> Result<(String, Vec<ImportedImage>), String> {
    let source = from
        .parent()
        .ok_or("Invalid source document path")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut replacements = Vec::new();
    let mut images = Vec::new();
    for (event, range) in Parser::new(markdown).into_offset_iter() {
        let Event::Start(Tag::Image { dest_url, .. }) = event else {
            continue;
        };
        let Some(path) = crate::asset_policy::target(&source, &dest_url) else {
            continue;
        };
        if !managed_filename(&path) {
            continue;
        }
        let path = path
            .canonicalize()
            .map_err(|e| format!("Could not locate imported image: {e}"))?;
        if !path.starts_with(&source) && !known.contains(&path) {
            return Err("This external image is not registered as a Markora import.".into());
        }
        let input = read_inputs(std::slice::from_ref(&path))?.remove(0);
        let extension = validate(&input.bytes)?;
        if path.file_name().unwrap_or_default()
            != format!("image-{}.{}", hash(&input.bytes), extension).as_str()
        {
            return Err("An imported image was modified; restore it before Save As.".into());
        }
        let imported = super::import_with_policy(to, vec![input], policy)?.remove(0);
        let slice = &markdown[range.clone()];
        let offset = slice.rfind(dest_url.as_ref()).ok_or(
            "Could not safely rewrite this image reference. Use an inline image link for Save As.",
        )?;
        replacements.push((
            range.start + offset..range.start + offset + dest_url.len(),
            crate::asset_policy::encode_link(&imported.relative_path),
        ));
        images.push(imported);
    }
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut text = markdown.to_string();
    for (range, url) in replacements {
        text.replace_range(range, &url);
    }
    Ok((text, images))
}

fn managed_path(url: &str) -> Option<PathBuf> {
    let decoded = crate::media::percent_decode(url)?;
    let parts: Vec<_> = decoded.split('/').collect();
    if parts.len() != 2 || !parts[0].ends_with(".assets") || parts[0].contains(['\\', ':']) {
        return None;
    }
    let (name, ext) = parts[1].rsplit_once('.')?;
    let digest = name.strip_prefix("image-")?;
    if digest.len() != 64
        || !digest.bytes().all(|b| b.is_ascii_hexdigit())
        || !matches!(ext, "png" | "jpg" | "gif" | "webp")
    {
        return None;
    }
    let path = PathBuf::from(decoded);
    path.components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
        .then_some(path)
}

pub fn copy_references(markdown: &str, from: &Path, to: &Path) -> Result<(), String> {
    let source = from
        .parent()
        .ok_or("Invalid source document path")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let destination = to
        .parent()
        .ok_or("Invalid save destination")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if source == destination {
        return Ok(());
    }
    let paths: HashSet<_> = Parser::new(markdown)
        .filter_map(|event| match event {
            Event::Start(Tag::Image { dest_url, .. }) => managed_path(&dest_url),
            _ => None,
        })
        .collect();
    for relative in paths {
        let path = source
            .join(&relative)
            .canonicalize()
            .map_err(|e| format!("Could not copy imported image {}: {e}", relative.display()))?;
        if !path.starts_with(&source) {
            return Err("An imported image points outside the source directory.".into());
        }
        let input = read_inputs(&[path])?.remove(0);
        let extension = validate(&input.bytes)?;
        let expected = format!("image-{}.{}", hash(&input.bytes), extension);
        if relative.file_name().unwrap_or_default() != expected.as_str() {
            return Err("An imported image was modified; restore it before Save As.".into());
        }
        store(&destination, &relative, &input.bytes)?;
    }
    Ok(())
}
