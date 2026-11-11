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
    storage::write_file(&dir.join("spec.pdf"), b"%PDF-1.4\n").unwrap();
    storage::write_file(&dir.join("ignored.exe"), b"ignored").unwrap();
    assert_eq!(
        storage::read_markdown(&dir.join("नोट.md")).unwrap(),
        "# नमस्ते 😀"
    );
    let entries = storage::folder_entries(&dir).unwrap();
    assert_eq!(entries.len(), 3);
    assert!(entries[0].is_folder);
    assert_eq!(entries[1].name, "spec.pdf");
    assert_eq!(entries[2].name, "नोट.md");
    assert_eq!(
        storage::resolve_relative_link(Some(&dir), "spec.pdf#page=2").unwrap(),
        Some(dir.join("spec.pdf"))
    );
    assert!(storage::resolve_relative_link(Some(&dir), "ignored.exe").is_err());
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

#[test]
fn storage_discover_uses_binary_directory_and_respects_overrides() {
    let dir = test_dir();
    let bin_dir = dir.join("bin");
    let work_dir = dir.join("notes");
    let override_dir = dir.join("override");
    let app_data = dir.join("appdata");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&work_dir).unwrap();

    let exe_path = bin_dir.join("markora.exe");

    // 1. Explicit MARKORA_DATA_DIR override wins.
    let overridden = Storage::discover_with(
        Some(override_dir.clone()),
        Some(exe_path.clone()),
        Some(app_data.clone()),
    );
    assert_eq!(overridden.dir, override_dir);
    assert!(!bin_dir.join(".markora").exists());

    // 2. Without override, `.markora` is created alongside the binary, not in `work_dir`.
    let portable = Storage::discover_with(None, Some(exe_path), Some(app_data.clone()));
    assert_eq!(portable.dir, bin_dir.join(".markora"));
    assert!(bin_dir.join(".markora").is_dir());
    assert!(!work_dir.join(".markora").exists());

    // 3. When the binary directory cannot hold `.markora`, fall back to APPDATA/Markora.
    let blocker = dir.join("not-a-dir");
    std::fs::write(&blocker, b"file").unwrap();
    let fallback = Storage::discover_with(
        None,
        Some(blocker.join("markora.exe")),
        Some(app_data.clone()),
    );
    assert_eq!(fallback.dir, app_data.join("Markora"));

    let no_exe = Storage::discover_with(None, None, Some(app_data.clone()));
    assert_eq!(no_exe.dir, app_data.join("Markora"));

    std::fs::remove_dir_all(dir).unwrap();
}
