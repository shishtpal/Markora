//! Exercise production models and protocol, never mirrors of implementation code.
use markora::{
    markdown::render,
    model::{Document, Prefs, ViewMode, Workspace},
    protocol::{Action, PrefsPatch, Request},
};

#[test]
fn command_draft_roundtrips_unicode_and_windows_paths() {
    let markdown = "# नमस्ते 😀\nC:\\notes\\\"draft\"\n<script>";
    let request = serde_json::json!({"type":"command", "command":"save", "draft": {
        "docId": 9, "markdown": markdown, "revision": 42
    }});
    let parsed: Request = serde_json::from_value(request).unwrap();
    assert!(matches!(parsed.action, Action::Command { command } if command == "save"));
    assert_eq!(parsed.draft.unwrap().markdown, markdown);
}

#[test]
fn malformed_and_unknown_messages_return_errors() {
    assert!(serde_json::from_str::<Request>(r#"{"type":"command"}"#).is_err());
    assert!(serde_json::from_str::<Request>(r#"{"type":"missing"}"#).is_err());
}

#[test]
fn settings_migrate_old_preferences_and_validate_layout() {
    let mut prefs: Prefs =
        serde_json::from_str(r#"{"colorMode":"light","viewMode":"split"}"#).unwrap();
    assert_eq!(prefs.files_width, 240.0);
    let patch: PrefsPatch = serde_json::from_str(
        r#"{"filesWidth":99999,"outlineWidth":1,"splitRatio":2,"mdTheme":"missing"}"#,
    )
    .unwrap();
    patch.apply(&mut prefs);
    assert_eq!(prefs.files_width, 560.0);
    assert_eq!(prefs.outline_width, 160.0);
    assert_eq!(prefs.split_ratio, 0.8);
    assert_eq!(prefs.md_theme, "cobalt");
    assert_eq!(prefs.color_mode, "light");
    assert_eq!(prefs.view_mode, ViewMode::Split);
}

#[test]
fn stale_draft_cannot_replace_newer_document() {
    let mut doc = Document::new(1, "original".into(), ViewMode::Live);
    doc.edit("newest".into(), 10);
    doc.edit("late".into(), 9);
    assert_eq!(doc.markdown, "newest");
    assert!(doc.dirty());
    doc.edit("original".into(), 11);
    assert!(!doc.dirty());
}

#[test]
fn editing_inactive_tab_never_overwrites_active_tab() {
    let mut workspace = Workspace::new("first".into(), ViewMode::Live);
    workspace.add("second".into(), ViewMode::Source);
    workspace.edit(1, "first, edited".into(), 1);
    assert_eq!(workspace.doc().markdown, "second");
    assert_eq!(workspace.tabs[0].markdown, "first, edited");
    workspace.close(1, ViewMode::Live);
    assert_eq!(workspace.doc().id, 2);
    workspace.close(2, ViewMode::Live);
    assert_eq!(workspace.tabs.len(), 1);
    assert!(!workspace.doc().dirty());
}

#[test]
fn live_block_offsets_use_javascript_utf16_indexes() {
    let markdown = "# नमस्ते 😀\n\nParagraph **bold**.\n\n- One\n- Two\n";
    let rendered = render(markdown);
    assert_eq!(rendered.blocks.len(), 3);
    assert_eq!(rendered.outline[0].title, "नमस्ते 😀");
    let start = rendered.blocks[1].start;
    let utf16: Vec<u16> = markdown.encode_utf16().collect();
    let selected = String::from_utf16(&utf16[start..rendered.blocks[1].end]).unwrap();
    assert_eq!(selected.trim(), "Paragraph **bold**.");
    assert!(rendered.html.contains("<strong>bold</strong>"));
    assert!(rendered.blocks[2].html.contains("<ul>"));
}

#[test]
fn preview_and_live_support_gfm_without_executable_html() {
    let output = render(
        "<script>alert(1)</script>\n\n[x](javascript:alert%281%29)\n\n| A | B |\n|---|---|\n| ~~old~~ | new |\n\n- [x] Done\n",
    );
    assert!(!output.html.contains("<script>"));
    assert!(!output.html.contains("javascript:"));
    assert!(output.html.contains("<table>"));
    assert!(output.html.contains("<del>old</del>"));
    assert!(output.html.contains("checked"));
}

#[test]
fn inline_code_is_in_outline_and_nested_blocks_remain_intact() {
    let output =
        render("## Use `Rust`\n\n> Quote\n>\n> - Nested list\n\n```rust\nfn main() {}\n```\n");
    assert_eq!(output.outline[0].title, "Use Rust");
    assert_eq!(output.blocks.len(), 3);
    assert!(output.blocks[1].html.contains("<blockquote>"));
    assert!(output.blocks[1].html.contains("Nested list"));
    assert!(output.blocks[2].html.contains("language-rust"));
}

#[test]
fn audio_and_video_tags_are_sanitized_and_normalized() {
    let md = "<audio src=\"Todos.assets\\Krishna Madam 2021-09-16 09-50-50.m4a\" />\n\nAfter audio.\n\n<video src=\"clip.mp4\" width=\"640\" onerror=\"alert(1)\" />\n\n<audio src=\"javascript:alert(1)\" />\n\n<audio><script>alert(1)</script></audio>\n";
    let output = render(md);
    assert!(output.html.contains(
        "<audio controls preload=\"metadata\" src=\"Todos.assets/Krishna Madam 2021-09-16 09-50-50.m4a\"></audio>",
    ));
    assert!(output.html.contains("<p>After audio.</p>"));
    assert!(
        output.html.contains(
            "<video controls preload=\"metadata\" src=\"clip.mp4\" width=\"640\"></video>"
        )
    );
    assert!(!output.html.contains("onerror"));
    assert!(!output.html.contains("javascript:"));
    assert!(!output.html.contains("<script>"));
}

#[test]
fn audio_before_nested_todos_without_blank_line_renders_list() {
    let md = concat!(
        "<audio src=\"Todos.assets\\Krishna Madam 2021-09-16 09-50-50.m4a\" />\n",
        "- [ ] Krishna School Mam\n",
        "  - [x] TC format for counter signature\n",
        "  - [ ] Attendance Sheet\n",
        "  - [ ] ![](Todos.assets/21deab21-42e9-4c69-bef3-5b4e124de21b.jpg)\n",
        "<audio src=\"Todos.assets\\Kapil Goyal@sjbsvm Mathura 2021-09-16 10-18-21.m4a\" />\n",
        "- [ ] Kapil Goyal Mathura\n",
        "  - [ ] Manually Enter Fees Fine Amount\n",
    );
    let output = render(md);
    assert!(output.html.contains("<audio controls preload=\"metadata\" src=\"Todos.assets/Krishna Madam 2021-09-16 09-50-50.m4a\"></audio>"));
    assert!(output.html.contains("<audio controls preload=\"metadata\" src=\"Todos.assets/Kapil Goyal@sjbsvm Mathura 2021-09-16 10-18-21.m4a\"></audio>"));
    assert!(output.html.contains("<ul>"));
    assert!(
        output
            .html
            .contains("<input disabled=\"\" type=\"checkbox\"/>")
    );
    assert!(output.html.contains("Krishna School Mam"));
    assert!(output.html.contains("Kapil Goyal Mathura"));
    assert_eq!(output.blocks.len(), 2);
}

#[test]
fn large_markdown_renders_in_linear_time_with_accurate_utf16_offsets() {
    let section = "## Section नमस्ते 😀\n\nParagraph with **bold**, `code`, and [ref][link].\n\n- Item one\n- Item two\n\n";
    let mut md = String::with_capacity(section.len() * 6000 + 64);
    for _ in 0..6000 {
        md.push_str(section);
    }
    md.push_str("[link]: https://example.com\n");
    let start = std::time::Instant::now();
    let rendered = render(&md);
    let elapsed = start.elapsed();
    assert_eq!(rendered.outline.len(), 6000);
    assert!(rendered.blocks.len() >= 18000);
    assert!(rendered.html.contains("href=\"https://example.com\""));
    let last_heading = rendered.outline.last().unwrap();
    let tail_utf16: Vec<u16> = md.encode_utf16().collect();
    let slice =
        String::from_utf16(&tail_utf16[last_heading.start..last_heading.start + 20]).unwrap();
    assert!(slice.starts_with("## Section नमस्ते 😀"));
    assert!(
        elapsed.as_secs_f64() < 3.0,
        "2MB render took too long: {elapsed:?}"
    );
}

#[test]
fn source_mode_defers_full_render_until_mode_switch() {
    let md = "# Heading\n\n".repeat(6000);
    let mut doc = Document::new(1, md, ViewMode::Source);
    assert!(!doc.rendered_full);
    assert!(doc.rendered.blocks.is_empty());
    assert_eq!(doc.rendered.outline.len(), 6000);
    assert!(!doc.dirty());
    doc.set_view_mode(ViewMode::Live);
    assert!(doc.rendered_full);
    assert_eq!(doc.rendered.blocks.len(), 6000);
    assert!(doc.rendered.html.contains("<h1>Heading</h1>"));
}

#[test]
fn cached_counts_preserve_unicode_and_ascii_whitespace_semantics() {
    for markdown in [
        "",
        "one\u{b}two\tthree\r\nfour",
        "नमस्ते 😀\u{a0}世界",
        "a\u{2003}b",
    ] {
        assert_eq!(
            markora::markdown::text_counts(markdown),
            (
                markdown.split_whitespace().count(),
                markdown.chars().count()
            )
        );
    }
}

#[test]
fn saving_and_undoing_preserve_the_new_clean_baseline() {
    let mut doc = Document::new(1, "original".into(), ViewMode::Live);
    doc.edit("saved".into(), 1);
    doc.mark_saved();
    assert!(!doc.dirty());
    doc.edit("original".into(), 2);
    assert!(doc.dirty());
    doc.edit("saved".into(), 3);
    assert!(!doc.dirty());
    doc.edit("changed again".into(), 4);
    assert!(doc.dirty());
}
