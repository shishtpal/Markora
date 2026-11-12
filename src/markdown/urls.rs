//! Image file URLs are later resolved through the imported-file allowlist.
use pulldown_cmark::CowStr;

pub(super) fn safe_image_url(url: CowStr<'_>) -> CowStr<'_> {
    if url.starts_with("file:") && url::Url::parse(&url).is_ok_and(|u| u.to_file_path().is_ok()) {
        url
    } else {
        safe_url(url)
    }
}

pub(super) fn safe_url(url: CowStr<'_>) -> CowStr<'_> {
    if url.is_ascii() && !url.contains(':') && !url.bytes().any(|b| b.is_ascii_control()) {
        return url;
    }
    let normalized: String = url
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .collect();
    let lower = normalized.to_ascii_lowercase();
    if lower.contains(':')
        && !["https:", "http:", "mailto:"]
            .iter()
            .any(|p| lower.starts_with(p))
    {
        "#".into()
    } else {
        url
    }
}
