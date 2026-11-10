//! Document exporters for HTML, Rich Text (RTF), and plain text.
use crate::{markdown::options, protocol::HtmlStyle};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn theme_palette(
    color_mode: &str,
    md_theme: &str,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    let dark = color_mode == "dark";
    match (md_theme, dark) {
        ("paper", false) => (
            "#fcfaf4", "#2a241d", "#706658", "#e7e0d2", "#f3efe4", "#9e4722",
        ),
        ("paper", true) => (
            "#1b1916", "#efe9de", "#b0a594", "#37322a", "#25221d", "#e5986e",
        ),
        ("graphite", false) => (
            "#f8f9fa", "#1f2328", "#5e6670", "#dde1e6", "#edf0f3", "#3d4856",
        ),
        ("graphite", true) => (
            "#16181c", "#e6e9ef", "#9aa3b2", "#2e333b", "#202329", "#aeb8c7",
        ),
        ("forest", false) => (
            "#f5f9f6", "#152b20", "#4b6e5c", "#d6e4db", "#e9f2ec", "#0f764a",
        ),
        ("forest", true) => (
            "#101b16", "#e2f2e9", "#8cb39f", "#263d32", "#182921", "#6ce0a9",
        ),
        ("mars", false) => (
            "#fdf7f5", "#2e1915", "#7d524a", "#efdcd7", "#f7eae6", "#b83b28",
        ),
        ("mars", true) => (
            "#1c1312", "#f7e9e6", "#c49890", "#3e2926", "#291b19", "#ff8b78",
        ),
        ("github", false) => (
            "#ffffff", "#1f2328", "#59636e", "#d1d9e0", "#f6f8fa", "#0969da",
        ),
        ("github", true) => (
            "#0d1117", "#f0f6fc", "#9198a1", "#3d444d", "#151b23", "#4493f8",
        ),
        (_, true) => (
            "#101722", "#e8efff", "#a5b5d1", "#2d3e56", "#1d2a3b", "#6f9bff",
        ),
        (_, false) => (
            "#f4f7fc", "#10234b", "#5c6f91", "#e0e7f2", "#eef3fb", "#1246c5",
        ),
    }
}

pub fn to_html(
    title: &str,
    body_html: &str,
    color_mode: &str,
    md_theme: &str,
    style: HtmlStyle,
) -> String {
    let mode = if matches!(style, HtmlStyle::Light) {
        "light"
    } else {
        color_mode
    };
    let (bg, text, muted, divider, code_bg, accent) = theme_palette(mode, md_theme);
    let font_body = match md_theme {
        "paper" if !matches!(style, HtmlStyle::Minimal) => "Georgia, 'Times New Roman', serif",
        "mono" if !matches!(style, HtmlStyle::Minimal) => {
            "ui-monospace, 'Cascadia Mono', Consolas, monospace"
        }
        _ => "system-ui, -apple-system, 'Segoe UI', sans-serif",
    };
    let css = if matches!(style, HtmlStyle::Minimal) {
        "body{max-width:780px;margin:40px auto;padding:0 24px;font:16px/1.7 system-ui,sans-serif}pre{overflow:auto;padding:12px;background:#f5f5f5}code{font-family:monospace}table{border-collapse:collapse;width:100%}td,th{border:1px solid #ddd;padding:6px 10px;text-align:left}blockquote{margin:1em 0;padding-left:1em;border-left:3px solid #ccc}".to_string()
    } else {
        format!(
            ":root{{color-scheme:{mode};--bg:{bg};--text:{text};--muted:{muted};--div:{divider};--code:{code_bg};--acc:{accent}}}*{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:var(--text);font:400 16px/1.75 {font_body}}}main{{max-width:820px;margin:0 auto;padding:48px 28px 64px}}h1,h2,h3,h4{{line-height:1.28;margin:1.3em 0 .5em}}h1{{font-size:2rem;margin-top:0}}h2{{font-size:1.45rem;border-bottom:1px solid var(--div);padding-bottom:.3em}}a{{color:var(--acc)}}code{{font:.88em/1.5 ui-monospace,Consolas,monospace;background:var(--code);border:1px solid var(--div);border-radius:4px;padding:1px 5px}}pre{{background:var(--code);border:1px solid var(--div);border-radius:7px;padding:14px 18px;overflow:auto}}pre code{{border:0;padding:0;background:none}}blockquote{{margin:1em 0;padding:10px 18px;border-left:3px solid var(--acc);color:var(--muted);background:color-mix(in srgb,var(--acc) 10%,transparent);border-radius:0 6px 6px 0}}blockquote>:first-child{{margin-top:0}}blockquote>:last-child{{margin-bottom:0}}table{{width:100%;border-collapse:collapse;margin:1.1em 0}}td,th{{border:1px solid var(--div);padding:8px 12px;text-align:left}}th{{background:var(--code)}}img{{max-width:100%;height:auto}}hr{{border:0;border-top:1px solid var(--div);margin:1.6em 0}}li:has(>input[type=checkbox]){{list-style:none;margin-left:-1.3em}}@media print{{body{{background:#fff;color:#111}}main{{max-width:none;padding:0}}}}",
        )
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n<title>{}</title>\n<style>{css}</style>\n</head>\n<body>\n<main class=\"markdown-body\">\n{body_html}</main>\n</body>\n</html>\n",
        escape_html(title),
    )
}

pub fn to_plain_text(markdown: &str) -> String {
    let mut out = String::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut quote_depth = 0usize;
    let mut link_url: Option<String> = None;
    let mut link_text = String::new();
    let mut in_table_row = false;
    let mut first_cell = true;

    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::List(start)) => lists.push(start),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                if lists.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => {
                for _ in 1..lists.len() {
                    out.push_str("  ");
                }
                match lists.last_mut() {
                    Some(Some(num)) => {
                        out.push_str(&format!("{num}. "));
                        *num += 1;
                    }
                    _ => out.push_str("- "),
                }
            }
            Event::End(TagEnd::Item) if !out.ends_with('\n') => out.push('\n'),
            Event::Start(Tag::BlockQuote(_)) => quote_depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => {
                quote_depth = quote_depth.saturating_sub(1);
                out.push('\n');
            }
            Event::Start(Tag::Paragraph | Tag::Heading { .. } | Tag::CodeBlock(_))
                if quote_depth > 0 && (out.is_empty() || out.ends_with('\n')) =>
            {
                out.push_str(&"> ".repeat(quote_depth));
            }
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock) => {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                if lists.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::TableRow) | Event::Start(Tag::TableHead) => {
                in_table_row = true;
                first_cell = true;
            }
            Event::End(TagEnd::TableRow | TagEnd::TableHead) => {
                in_table_row = false;
                out.push('\n');
            }
            Event::Start(Tag::TableCell) if in_table_row => {
                if !first_cell {
                    out.push_str(" | ");
                }
                first_cell = false;
            }
            Event::End(TagEnd::Table) => out.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_url = Some(dest_url.into_string());
                link_text.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = link_url.take()
                    && !url.is_empty()
                    && url != link_text
                    && url != "#"
                {
                    out.push_str(&format!(" ({url})"));
                }
            }
            Event::TaskListMarker(checked) => {
                out.push_str(if checked { "[x] " } else { "[ ] " });
            }
            Event::Text(text) | Event::Code(text) => {
                if link_url.is_some() {
                    link_text.push_str(&text);
                }
                out.push_str(&text);
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            Event::Rule => out.push_str("---\n\n"),
            _ => {}
        }
    }
    out.trim_end().to_string() + "\n"
}

fn push_rtf_text(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\n' => out.push_str("\\line "),
            c if c.is_ascii() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{}?", *unit as i16));
                }
            }
        }
    }
}

pub fn to_rtf(title: &str, markdown: &str) -> String {
    let mut out = String::from(
        "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fswiss Segoe UI;}{\\f1\\fmodern Consolas;}}{\\colortbl;\\red16\\green35\\blue75;\\red92\\green111\\blue145;}{\\info{\\title ",
    );
    push_rtf_text(&mut out, title);
    out.push_str("}}\\fs22\\cf1 ");

    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let size = match level as u8 {
                    1 => 40,
                    2 => 32,
                    3 => 26,
                    _ => 24,
                };
                out.push_str(&format!("\\pard\\sa140\\sb180\\b\\fs{size} "));
            }
            Event::End(TagEnd::Heading(_)) => out.push_str("\\b0\\fs22\\par\n"),
            Event::Start(Tag::Paragraph) if lists.is_empty() => out.push_str("\\pard\\sa140 "),
            Event::End(TagEnd::Paragraph) if lists.is_empty() => out.push_str("\\par\n"),
            Event::Start(Tag::BlockQuote(_)) => out.push_str("\\pard\\li360\\cf2 "),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("\\cf1\\par\n"),
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                out.push_str("\\pard\\li240\\sa140\\f1\\fs20 ");
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                out.push_str("\\f0\\fs22\\par\n");
            }
            Event::Start(Tag::List(start)) => lists.push(start),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                let indent = lists.len() * 360;
                out.push_str(&format!("\\pard\\li{indent}\\sa60 "));
                match lists.last_mut() {
                    Some(Some(num)) => {
                        out.push_str(&format!("{num}. "));
                        *num += 1;
                    }
                    _ => out.push_str("\\bullet  "),
                }
            }
            Event::End(TagEnd::Item) => out.push_str("\\par\n"),
            Event::Start(Tag::Strong) => out.push_str("\\b "),
            Event::End(TagEnd::Strong) => out.push_str("\\b0 "),
            Event::Start(Tag::Emphasis) => out.push_str("\\i "),
            Event::End(TagEnd::Emphasis) => out.push_str("\\i0 "),
            Event::Start(Tag::Strikethrough) => out.push_str("\\strike "),
            Event::End(TagEnd::Strikethrough) => out.push_str("\\strike0 "),
            Event::TaskListMarker(checked) => {
                out.push_str(if checked { "[x] " } else { "[ ] " });
            }
            Event::Code(text) => {
                if !in_code_block {
                    out.push_str("{\\f1 ");
                    push_rtf_text(&mut out, &text);
                    out.push('}');
                } else {
                    push_rtf_text(&mut out, &text);
                }
            }
            Event::Text(text) => push_rtf_text(&mut out, &text),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str("\\line "),
            Event::Rule => out.push_str("\\pard\\brdrb\\brdrs\\brdrw10\\brsp20\\par\n"),
            _ => {}
        }
    }
    out.push('}');
    out
}
