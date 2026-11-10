//! One CommonMark/GFM parser supplies preview HTML, live blocks and outline.
//! Offsets cross into JavaScript as UTF-16 positions, never UTF-8 byte indexes.
use crate::html_media::{sanitize_media_html, split_media_prefix};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use serde::Serialize;

mod counts;
pub use counts::text_counts;

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

#[derive(Debug, Default, Serialize)]
pub struct RenderedDocument {
    #[serde(skip_serializing)]
    pub html: String,
    pub blocks: Vec<Block>,
    pub outline: Vec<Heading>,
}

struct RawBlock {
    start: usize,
    end: usize,
    html: String,
}

struct Utf16Cursor<'a> {
    text: &'a str,
    byte: usize,
    utf16: usize,
}

impl<'a> Utf16Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            byte: 0,
            utf16: 0,
        }
    }

    fn advance_to(&mut self, target: usize) -> usize {
        if target < self.byte {
            return self.text[..target].encode_utf16().count();
        }
        if target > self.byte {
            let slice = &self.text[self.byte..target];
            self.utf16 += if slice.is_ascii() {
                slice.len()
            } else {
                slice.encode_utf16().count()
            };
            self.byte = target;
        }
        self.utf16
    }
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

fn flush_block(
    blocks: &mut Option<&mut Vec<RawBlock>>,
    pending_events: &mut Vec<Event<'_>>,
    start: usize,
    end: usize,
) {
    let Some(out_blocks) = blocks.as_deref_mut() else {
        pending_events.clear();
        return;
    };
    let mut html = String::new();
    html::push_html(&mut html, pending_events.drain(..));
    out_blocks.push(RawBlock { start, end, html });
}

fn collect_slice(
    markdown: &str,
    base: usize,
    end: usize,
    blocks: &mut Option<&mut Vec<RawBlock>>,
    outline: &mut Vec<Heading>,
    heading_cursor: &mut Utf16Cursor<'_>,
) {
    let mut depth = 0usize;
    let mut block_start = base;
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
            && let Some((mut clean_media, split_rel)) =
                split_media_prefix(&markdown[abs_start..abs_end])
        {
            if let Some(out_blocks) = blocks.as_deref_mut() {
                if !clean_media.ends_with('\n') {
                    clean_media.push('\n');
                }
                out_blocks.push(RawBlock {
                    start: abs_start,
                    end: abs_start + split_rel,
                    html: clean_media,
                });
            }
            collect_slice(
                markdown,
                abs_start + split_rel,
                abs_end,
                blocks,
                outline,
                heading_cursor,
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
                        start: heading_cursor.advance_to(start),
                    });
                }
            }
            _ => {}
        }
        let is_start = matches!(event, Event::Start(_));
        let is_end = matches!(event, Event::End(_));
        if is_start && depth == 0 {
            block_start = abs_start;
        }
        if blocks.is_some() {
            pending_events.push(safe_event(event));
        }
        if is_start {
            depth += 1;
        } else if is_end {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                flush_block(blocks, &mut pending_events, block_start, abs_end);
            }
        } else if depth == 0 {
            flush_block(blocks, &mut pending_events, abs_start, abs_end);
        }
    }
    if !pending_events.is_empty() {
        flush_block(blocks, &mut pending_events, block_start, end);
    }
}

pub fn render_outline(markdown: &str) -> Vec<Heading> {
    let mut outline = Vec::new();
    let mut cursor = Utf16Cursor::new(markdown);
    let mut no_blocks = None;
    collect_slice(
        markdown,
        0,
        markdown.len(),
        &mut no_blocks,
        &mut outline,
        &mut cursor,
    );
    outline
}

pub fn render(markdown: &str) -> RenderedDocument {
    let mut raw_blocks = Vec::new();
    let mut outline = Vec::new();
    let mut heading_cursor = Utf16Cursor::new(markdown);
    let mut some_blocks = Some(&mut raw_blocks);
    collect_slice(
        markdown,
        0,
        markdown.len(),
        &mut some_blocks,
        &mut outline,
        &mut heading_cursor,
    );

    let total_html_len: usize = raw_blocks.iter().map(|b| b.html.len()).sum();
    let mut html = String::with_capacity(total_html_len);
    let mut block_cursor = Utf16Cursor::new(markdown);
    let mut blocks = Vec::with_capacity(raw_blocks.len());
    for b in raw_blocks {
        html.push_str(&b.html);
        blocks.push(Block {
            start: block_cursor.advance_to(b.start),
            end: block_cursor.advance_to(b.end),
            html: b.html,
        });
    }
    RenderedDocument {
        html,
        blocks,
        outline,
    }
}
