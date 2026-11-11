use markora::pdf::{PdfDocument, validate_header};
use std::sync::Arc;

fn minimal_pdf(text: &str) -> Vec<u8> {
    let stream = format!("BT /F1 18 Tf 72 720 Td ({text}) Tj ET");
    let objects = [
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_string(),
        "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_string(),
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n".to_string(),
        format!(
            "4 0 obj\n<< /Length {} >>\nstream\n{stream}\nendstream\nendobj\n",
            stream.len()
        ),
        "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for obj in &objects {
        offsets.push(out.len());
        out.extend_from_slice(obj.as_bytes());
    }
    let xref_pos = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[test]
fn parses_renders_and_extracts_text_from_pdf() {
    let bytes = minimal_pdf("Hello Markora PDF");
    assert!(validate_header(&bytes).is_ok());
    assert!(validate_header(b"not a pdf").is_err());

    let doc = PdfDocument::parse(Arc::new(bytes)).unwrap();
    assert_eq!(doc.page_count(), 1);
    assert_eq!(doc.page_sizes(), &[(612.0, 792.0)]);

    let rendered = doc.render_page(0, 0.5).unwrap();
    assert_eq!(rendered.width, 306);
    assert_eq!(rendered.height, 396);
    assert_eq!(rendered.rgba.len(), 306 * 396 * 4);
    assert!(rendered.rgba.chunks_exact(4).any(|px| px[0] < 250));

    let texts = doc.extract_page_texts();
    assert_eq!(texts.len(), 1);
    assert!(texts[0].contains("Hello Markora PDF"));
}
