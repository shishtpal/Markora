//! One CommonMark/GFM parser supplies preview HTML, live blocks and outline.
//! Offsets cross into JavaScript as UTF-16 positions, never UTF-8 byte indexes.
use crate::html_media::{sanitize_media_html, split_media_prefix};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub html: String,
}

#[derive(Debug, Serialize)]
pub struct Heading {
    pub level: u8,
    pub title: String,
    pub start: usize,
}

#[derive(Debug, Serialize)]
pub struct RenderedDocument {
    pub html: String,
    pub blocks: Vec<Block>,
    pub outline: Vec<Heading>,
}

pub fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

fn safe_event(event: Event<'_>) -> Event<'_> {
    // Markdown files are untrusted input: only sanitized <audio>/<video>/<source> pass as HTML.
    match event {
        Event::Html(text) => match sanitize_media_html(&text) {
            Some(clean) => Event::Html(clean.into()),
            None => Event::Text(text),
        },
        Event::InlineHtml(text) => match sanitize_media_html(&text) {
            Some(clean) => Event::InlineHtml(clean.into()),
            None => Event::Text(text),
        },
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: safe_url(dest_url),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: safe_url(dest_url),
            title,
            id,
        }),
        other => other,
    }
}

fn safe_url(url: pulldown_cmark::CowStr<'_>) -> pulldown_cmark::CowStr<'_> {
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

fn collect_slice(
    markdown: &str,
    base: usize,
    end: usize,
    blocks: &mut Vec<(usize, usize)>,
    outline: &mut Vec<Heading>,
    html_out: &mut String,
) {
    let mut depth = 0;
    let mut block_start = 0;
    let mut heading: Option<(u8, String, usize)> = None;
    let mut pending_events = Vec::new();
    let mut skip_html_block = false;
    for (event, rel) in Parser::new_ext(&markdown[base..end], options()).into_offset_iter() {
        if skip_html_block {
            if matches!(event, Event::End(TagEnd::HtmlBlock)) {
                skip_html_block = false;
            }
            continue;
        }
        let abs_start = base + rel.start;
        let abs_end = base + rel.end;
        if depth == 0
            && matches!(event, Event::Start(Tag::HtmlBlock))
            && let Some((clean_media, split_rel)) = split_media_prefix(&markdown[abs_start..abs_end])
        {
            if !pending_events.is_empty() {
                html::push_html(html_out, pending_events.drain(..));
            }
            blocks.push((abs_start, abs_start + split_rel));
            html_out.push_str(&clean_media);
            if !clean_media.ends_with('\n') {
                html_out.push('\n');
            }
            collect_slice(
                markdown,
                abs_start + split_rel,
                abs_end,
                blocks,
                outline,
                html_out,
            );
            skip_html_block = true;
            continue;
        }
        match &event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((*level as u8, String::new(), abs_start));
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, title, _)) = heading.as_mut() {
                    title.push_str(text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, title, _)) = heading.as_mut() {
                    title.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, title, start)) = heading.take() {
                    outline.push(Heading {
                        level,
                        title,
                        start: utf16(markdown, start),
                    });
                }
            }
            _ => {}
        }
        match &event {
            Event::Start(_) => {
                if depth == 0 {
                    block_start = abs_start;
                }
                depth += 1;
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    blocks.push((block_start, abs_end));
                }
            }
            _ if depth == 0 => blocks.push((abs_start, abs_end)),
            _ => {}
        }
        pending_events.push(safe_event(event));
    }
    if !pending_events.is_empty() {
        html::push_html(html_out, pending_events.into_iter());
    }
}

fn fragment(markdown: &str) -> String {
    let mut output = String::new();
    let mut blocks = Vec::new();
    let mut outline = Vec::new();
    collect_slice(markdown, 0, markdown.len(), &mut blocks, &mut outline, &mut output);
    output
}

pub fn render(markdown: &str) -> RenderedDocument {
    let mut raw_blocks = Vec::new();
    let mut outline = Vec::new();
    let mut html = String::new();
    collect_slice(markdown, 0, markdown.len(), &mut raw_blocks, &mut outline, &mut html);
    let blocks = raw_blocks
        .into_iter()
        .map(|(start, end)| Block {
            start: utf16(markdown, start),
            end: utf16(markdown, end),
            html: fragment(&markdown[start..end]),
        })
        .collect();
    RenderedDocument {
        html,
        blocks,
        outline,
    }
}

fn utf16(text: &str, byte_index: usize) -> usize {
    text[..byte_index].encode_utf16().count()
}
