use markora::{
    model::Prefs,
    storage::{self, Storage},
};
use std::path::PathBuf;

fn test_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-data")
        .join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn real_file_roundtrip_and_directory_filtering() {
    let dir = test_dir();
    std::fs::create_dir(dir.join("folder")).unwrap();
    storage::write_file(&dir.join("नोट.md"), "# नमस्ते 😀".as_bytes()).unwrap();
    storage::write_file(&dir.join("ignored.exe"), b"ignored").unwrap();
    assert_eq!(
        storage::read_markdown(&dir.join("नोट.md")).unwrap(),
        "# नमस्ते 😀"
    );
    let entries = storage::folder_entries(&dir).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].is_folder);
    assert_eq!(entries[1].name, "नोट.md");
    assert!(storage::read_markdown(&dir.join("missing.md")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn preferences_and_recents_are_real_json() {
    let dir = test_dir();
    let storage = Storage { dir: dir.clone() };
    let prefs = Prefs {
        files_width: 320.0,
        ..Default::default()
    };
    storage.write("prefs.json", &prefs).unwrap();
    assert_eq!(storage.prefs().files_width, 320.0);
    let mut recents = Vec::new();
    for index in 0..15 {
        storage::touch_recent(
            &mut recents,
            dir.join(format!("{index}.md")),
            index.to_string(),
        );
    }
    storage::touch_recent(&mut recents, dir.join("14.md"), "again".into());
    assert_eq!(recents.len(), 12);
    assert_eq!(recents[0].title, "again");
    storage.write("recents.json", &recents).unwrap();
    let restored: Vec<markora::model::Recent> = storage.read("recents.json");
    assert_eq!(restored[0].path, dir.join("14.md"));
    std::fs::write(dir.join("prefs.json"), "invalid JSON").unwrap();
    assert_eq!(storage.prefs().files_width, 240.0);
    std::fs::remove_dir_all(dir).unwrap();
}
