//! Production transfer and save-path regressions without a native window.
use super::tests::Fixture;
use base64::{Engine, engine::general_purpose::STANDARD};
use markora::image_protocol::{FileSpec, ImageRequest};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

fn fixture() -> (Fixture, Vec<u8>) {
    let mut f = Fixture::new();
    let path = f.dir.join("note.md");
    std::fs::write(&path, "original").unwrap();
    f.app.open_path(path);
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 2)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    (f, png.into_inner())
}
fn transfer(f: &mut Fixture, png: &[u8]) {
    let doc = f.app.workspace.doc();
    f.app.image_request(ImageRequest::Begin {
        token: 42,
        doc_id: doc.id,
        revision: doc.revision,
        files: vec![FileSpec {
            name: "shot.png".into(),
            size: png.len(),
        }],
    });
    for (seq, part) in [(&png[..20], false), (&png[20..], true)]
        .into_iter()
        .enumerate()
    {
        f.app.image_request(ImageRequest::Chunk {
            token: 42,
            seq,
            file_index: 0,
            data: STANDARD.encode(part.0),
            last: part.1,
        });
    }
}
fn finish(f: &mut Fixture) {
    let start = Instant::now();
    while f.app.images.reply.as_ref().unwrap().status == "working" {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
        f.app.drain_bg();
    }
}

#[test]
fn ordered_chunks_import_once_without_editing_the_document() {
    let (mut f, png) = fixture();
    transfer(&mut f, &png);
    assert_eq!(f.app.images.reply.as_ref().unwrap().next_seq, 2);
    f.app.image_request(ImageRequest::Finish { token: 42 });
    finish(&mut f);
    let reply = f.app.images.reply.as_ref().unwrap();
    assert_eq!(reply.status, "complete", "{}", reply.message);
    assert_eq!(reply.images.len(), 1);
    assert_eq!(
        std::fs::read(f.dir.join(&reply.images[0].relative_path)).unwrap(),
        png
    );
    assert_eq!(f.app.workspace.doc().markdown, "original");
    assert!(!f.app.workspace.doc().dirty());
}

#[test]
fn document_change_before_finish_prevents_writes_and_late_completion_is_stale() {
    let (mut f, png) = fixture();
    transfer(&mut f, &png);
    f.app.workspace.doc_mut().edit("changed".into(), 1);
    f.app.image_request(ImageRequest::Finish { token: 42 });
    assert_eq!(f.app.images.reply.as_ref().unwrap().status, "error");
    assert!(!f.dir.join("note.assets").exists());
    transfer(&mut f, &png);
    f.app.image_request(ImageRequest::Finish { token: 42 });
    f.app.workspace.doc_mut().edit("newest draft".into(), 2);
    finish(&mut f);
    assert_eq!(f.app.images.reply.as_ref().unwrap().status, "stale");
    assert_eq!(f.app.workspace.doc().markdown, "newest draft");
}

#[test]
fn unexpected_chunk_and_incomplete_file_are_rejected() {
    let (mut f, png) = fixture();
    let id = f.app.workspace.doc().id;
    f.app.image_request(ImageRequest::Begin {
        token: 42,
        doc_id: id,
        revision: 0,
        files: vec![FileSpec {
            name: "shot.png".into(),
            size: png.len(),
        }],
    });
    f.app.image_request(ImageRequest::Chunk {
        token: 42,
        seq: 3,
        file_index: 0,
        data: STANDARD.encode(&png),
        last: true,
    });
    assert_eq!(f.app.images.reply.as_ref().unwrap().status, "error");
    f.app.image_request(ImageRequest::Begin {
        token: 43,
        doc_id: id,
        revision: 0,
        files: vec![FileSpec {
            name: "shot.png".into(),
            size: png.len(),
        }],
    });
    f.app.image_request(ImageRequest::Chunk {
        token: 43,
        seq: 0,
        file_index: 0,
        data: STANDARD.encode(&png[..20]),
        last: true,
    });
    assert_eq!(f.app.images.reply.as_ref().unwrap().status, "error");
    assert!(!f.dir.join("note.assets").exists());
}

#[test]
fn save_as_copies_assets_before_marking_document_saved() {
    let (mut f, png) = fixture();
    transfer(&mut f, &png);
    f.app.image_request(ImageRequest::Finish { token: 42 });
    finish(&mut f);
    let path = f.app.images.reply.as_ref().unwrap().images[0]
        .relative_path
        .clone();
    let doc = f.app.workspace.doc_mut();
    doc.edit(format!("![Image]({path})"), 1);
    let id = doc.id;
    let destination = f.dir.join("copy");
    std::fs::create_dir(&destination).unwrap();
    assert!(f.app.save_to_path(id, destination.join("renamed.md")));
    let filename = std::path::Path::new(&path).file_name().unwrap().to_owned();
    assert_eq!(
        std::fs::read(destination.join("renamed.assets").join(&filename)).unwrap(),
        png
    );
    assert!(!f.app.workspace.doc().dirty());
    let next = f.dir.join("conflict");
    std::fs::create_dir_all(next.join("note.assets")).unwrap();
    std::fs::write(next.join(path), "unrelated").unwrap();
    f.app
        .workspace
        .doc_mut()
        .edit("![Image](note.assets/missing.png)".into(), 2);
    // Restore the managed link so a conflicting destination must fail before Markdown is saved.
    let link = format!("renamed.assets/{}", filename.to_string_lossy());
    f.app
        .workspace
        .doc_mut()
        .edit(format!("draft ![Image]({link})"), 3);
    assert!(!f.app.save_to_path(id, next.join("note.md")));
    assert!(!next.join("note.md").exists());
    assert!(f.app.workspace.doc().dirty());
}
