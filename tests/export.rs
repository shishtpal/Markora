use markora::{
    export::{to_html, to_plain_text, to_rtf},
    markdown,
    protocol::{Action, ExportFormat, HtmlStyle, Request},
};

#[test]
fn html_export_embeds_theme_escapes_title_and_keeps_unicode() {
    let md = "# नमस्ते 😀\n\nA **bold** paragraph with `code`.\n\n- [x] Done\n";
    let rendered = markdown::render(md);
    let html = to_html(
        "Doc <Title> & \"Test\"",
        &rendered.html,
        "dark",
        "forest",
        HtmlStyle::Current,
    );
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>Doc &lt;Title&gt; &amp; &quot;Test&quot;</title>"));
    assert!(html.contains("नमस्ते 😀"));
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("--bg:#101b16"));
    let light = to_html("Light", &rendered.html, "dark", "forest", HtmlStyle::Light);
    assert!(light.contains("--bg:#f5f9f6"));
    let minimal = to_html("Min", &rendered.html, "dark", "cobalt", HtmlStyle::Minimal);
    assert!(!minimal.contains("--bg:"));
}

#[test]
fn plain_text_export_strips_markdown_and_keeps_structure() {
    let md = "# Heading\n\nParagraph with **bold** and [link](https://example.com).\n\n- [x] Task\n- Item\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n";
    let txt = to_plain_text(md);
    assert!(txt.contains("Heading\n\n"));
    assert!(txt.contains("Paragraph with bold and link (https://example.com)."));
    assert!(txt.contains("- [x] Task\n- Item"));
    assert!(txt.contains("A | B\n1 | 2"));
}

#[test]
fn rtf_export_handles_formatting_and_unicode() {
    let md = "# Title\n\nHello **world** and *नमस्ते* 😀\n\n- One\n";
    let rtf = to_rtf("Title", md);
    assert!(rtf.starts_with("{\\rtf1\\ansi"));
    assert!(rtf.contains("\\b world\\b0"));
    assert!(rtf.contains("\\bullet  One"));
    assert!(rtf.contains("\\u2344?"));
    assert!(rtf.ends_with('}'));
}

#[test]
fn export_action_deserializes_with_defaults() {
    let req: Request = serde_json::from_str(r#"{"type":"export","format":"pdf"}"#).unwrap();
    assert!(matches!(
        req.action,
        Action::Export {
            format: ExportFormat::Pdf,
            html_style: HtmlStyle::Current,
        }
    ));
    let html_req: Request =
        serde_json::from_str(r#"{"type":"export","format":"html","htmlStyle":"minimal"}"#).unwrap();
    assert!(matches!(
        html_req.action,
        Action::Export {
            format: ExportFormat::Html,
            html_style: HtmlStyle::Minimal,
        }
    ));
}
