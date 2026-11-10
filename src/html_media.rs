//! Strict sanitizer and block splitter for embedded <audio>, <video>, and <source> tags.

fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn safe_media_src(value: &str) -> String {
    let trimmed = value.trim().replace('\\', "/");
    let compact: String = trimmed
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .collect();
    let lower = compact.to_ascii_lowercase();
    if lower.starts_with("//") {
        return "#".into();
    }
    if lower.contains(':')
        && ![
            "https:",
            "http:",
            "data:audio/",
            "data:video/",
            "data:image/",
        ]
        .iter()
        .any(|p| lower.starts_with(p))
    {
        return "#".into();
    }
    trimmed
}

fn sanitize_media_tag(inner: &str) -> Option<String> {
    let self_closing = inner.trim_end().ends_with('/');
    let body = inner.trim_end().strip_suffix('/').unwrap_or(inner).trim();
    if let Some(closing) = body.strip_prefix('/') {
        let name = closing.trim().to_ascii_lowercase();
        return matches!(name.as_str(), "audio" | "video").then(|| format!("</{name}>"));
    }
    let split = body.find(char::is_whitespace).unwrap_or(body.len());
    let name = body[..split].to_ascii_lowercase();
    if !matches!(name.as_str(), "audio" | "video" | "source") {
        return None;
    }
    let allowed: &[&str] = match name.as_str() {
        "audio" => &[
            "src", "controls", "preload", "loop", "muted", "autoplay", "title",
        ],
        "video" => &[
            "src", "poster", "controls", "preload", "loop", "muted", "autoplay", "width", "height",
            "title",
        ],
        _ => &["src", "type"],
    };
    let mut attrs = Vec::new();
    let mut rest = body[split..].trim_start();
    let (mut has_controls, mut has_preload) = (false, false);
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(rest.len());
        let attr = rest[..end].to_ascii_lowercase();
        if attr.is_empty() {
            return None;
        }
        rest = rest[end..].trim_start();
        let val = if let Some(after_eq) = rest.strip_prefix('=') {
            let after_eq = after_eq.trim_start();
            let (raw_val, next) =
                if let Some(q) = after_eq.chars().next().filter(|c| *c == '"' || *c == '\'') {
                    let tail = &after_eq[1..];
                    let close = tail.find(q)?;
                    (&tail[..close], &tail[close + 1..])
                } else {
                    let end = after_eq.find(char::is_whitespace).unwrap_or(after_eq.len());
                    (&after_eq[..end], &after_eq[end..])
                };
            rest = next.trim_start();
            Some(raw_val)
        } else {
            None
        };
        if !allowed.contains(&attr.as_str()) {
            continue;
        }
        if attr == "controls" {
            has_controls = true;
        } else if attr == "preload" {
            has_preload = true;
        }
        match val {
            Some(v) => {
                let clean = if attr == "src" || attr == "poster" {
                    safe_media_src(v)
                } else {
                    v.to_string()
                };
                attrs.push(format!("{attr}=\"{}\"", escape_attr(&clean)));
            }
            None => attrs.push(attr),
        }
    }
    let mut out = format!("<{name}");
    if name != "source" && !has_controls {
        out.push_str(" controls");
    }
    if name != "source" && !has_preload {
        out.push_str(" preload=\"metadata\"");
    }
    for attr in attrs {
        out.push(' ');
        out.push_str(&attr);
    }
    out.push('>');
    if self_closing && name != "source" {
        out.push_str(&format!("</{name}>"));
    }
    Some(out)
}

pub fn sanitize_media_html(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut found_tag = false;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('>')?;
        out.push_str(&sanitize_media_tag(&after[..close])?);
        found_tag = true;
        rest = &after[close + 1..];
    }
    if !found_tag {
        return None;
    }
    out.push_str(rest);
    Some(out)
}

/// If a CommonMark Type-7 HTML block starts with a self-contained <audio>/<video> line
/// followed by non-whitespace Markdown (no blank line in between), return the sanitized
/// media HTML and the byte offset where the trailing Markdown begins.
pub fn split_media_prefix(raw: &str) -> Option<(String, usize)> {
    let trimmed_start = raw.len() - raw.trim_start().len();
    let after_ws = &raw[trimmed_start..];
    let after_lt = after_ws.strip_prefix('<')?;
    let close = after_lt.find('>')?;
    let inner = &after_lt[..close];
    let lower = inner.trim_start().to_ascii_lowercase();
    let tag = if lower.starts_with("audio")
        && lower[5..].starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>')
    {
        "audio"
    } else if lower.starts_with("video")
        && lower[5..].starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>')
    {
        "video"
    } else {
        return None;
    };
    let mut elem_end = trimmed_start + 1 + close + 1;
    if !inner.trim_end().ends_with('/') {
        let close_pat = format!("</{tag}>");
        let rest_lower = raw[elem_end..].to_ascii_lowercase();
        let close_pos = rest_lower.find(&close_pat)?;
        elem_end += close_pos + close_pat.len();
    }
    let trailing_same_line = raw[elem_end..]
        .find('\n')
        .map(|i| elem_end + i + 1)
        .unwrap_or(raw.len());
    if !raw[elem_end..trailing_same_line].trim().is_empty()
        || raw[trailing_same_line..].trim().is_empty()
    {
        return None;
    }
    let clean = sanitize_media_html(&raw[..trailing_same_line])?;
    Some((clean, trailing_same_line))
}
