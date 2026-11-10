use markora::media::{MediaRoots, mime_type, resolve_media_path, serve_media};
use std::fs;

#[test]
fn resolves_local_media_with_spaces_and_blocks_traversal() {
    let dir = std::env::temp_dir().join(format!("markora-media-{}", std::process::id()));
    let assets = dir.join("Todos.assets");
    fs::create_dir_all(&assets).unwrap();
    let audio = assets.join("Krishna Madam 2021-09-16 09-50-50.m4a");
    fs::write(&audio, b"0123456789").unwrap();

    let mut roots = MediaRoots::default();
    roots.docs.insert(3, dir.clone());

    let found = resolve_media_path(
        &roots,
        "markora://localhost/doc/3/Todos.assets/Krishna%20Madam%202021-09-16%2009-50-50.m4a",
    )
    .unwrap();
    assert_eq!(found, audio.canonicalize().unwrap());
    assert!(resolve_media_path(&roots, "markora://localhost/doc/3/../secret.m4a").is_none());
    assert_eq!(mime_type(&found), "audio/mp4");

    let full = serve_media(
        &roots,
        "markora://localhost/doc/3/Todos.assets/Krishna%20Madam%202021-09-16%2009-50-50.m4a",
        None,
    );
    assert_eq!(full.status, 200);
    assert_eq!(full.body, b"0123456789");

    let partial = serve_media(
        &roots,
        "markora://localhost/doc/3/Todos.assets/Krishna%20Madam%202021-09-16%2009-50-50.m4a",
        Some("bytes=2-5"),
    );
    assert_eq!(partial.status, 206);
    assert_eq!(partial.content_range.as_deref(), Some("bytes 2-5/10"));
    assert_eq!(partial.body, b"2345");

    let _ = fs::remove_dir_all(&dir);
}
