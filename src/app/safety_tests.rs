//! Real file, restart and close-flow regressions for safety features.
use super::{tests::Fixture, *};
use markora::{
    file_io::FileVersion,
    model::ViewMode,
    protocol::{ConflictChoice, UnsavedChoice},
    recovery::{Session, SessionStore},
};

fn opened() -> (Fixture, PathBuf, u64) {
    let mut f = Fixture::new();
    let path = f.dir.join("notes.md");
    std::fs::write(&path, "original").unwrap();
    f.app.open_path(path.clone());
    let id = f.app.workspace.doc().id;
    (f, storage::normalize_path(&path), id)
}

#[test]
fn save_checks_disk_without_waiting_for_poll_and_keep_resumes_save() {
    let (mut f, path, id) = opened();
    f.app.workspace.doc_mut().edit("my draft".into(), 1);
    std::fs::write(&path, "external").unwrap();
    assert!(!f.app.save_document(id, false));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
    assert!(f.app.workspace.doc().dirty());
    let token = f.app.safety.conflict.as_ref().unwrap().token;
    f.request(
        serde_json::json!({"type":"resolveConflict","choice":"keep","token":token,"draft":{
            "docId":id,"revision":2,"markdown":"latest draft"
        }}),
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "latest draft");
    assert!(!f.app.workspace.doc().dirty());
    assert!(f.app.safety.conflict.is_none());
}

#[test]
fn changed_again_requires_new_review_and_reload_rejects_stale_drafts() {
    let (mut f, path, id) = opened();
    std::fs::write(&path, "first external").unwrap();
    assert!(!f.app.save_document(id, false));
    let token = f.app.safety.conflict.as_ref().unwrap().token;
    std::fs::write(&path, "second external").unwrap();
    f.request(serde_json::json!({"type":"resolveConflict","choice":"keep","token":token}));
    f.request(serde_json::json!({"type":"resolveConflict","choice":"keep","token":token}));
    assert!(f.app.safety.conflict.is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second external");
    f.app.resolve_conflict(ConflictChoice::Reload);
    assert_eq!(f.app.workspace.doc().markdown, "second external");
    assert!(!f.app.workspace.doc().dirty());
    f.request(
        serde_json::json!({"type":"edit","draft":{"docId":id,"markdown":"stale","revision":0}}),
    );
    assert_eq!(f.app.workspace.doc().markdown, "second external");
}

#[test]
fn poll_ignores_old_results_and_later_suppresses_only_that_version() {
    let (mut f, path, id) = opened();
    let baseline = f.app.workspace.doc().disk_version;
    std::fs::write(&path, "external").unwrap();
    let version = FileVersion::read(&path).unwrap();
    let observation = || safety::Observation {
        id,
        path: path.clone(),
        baseline,
        result: Ok(version),
    };
    f.app.apply_observations(vec![observation()]);
    assert!(f.app.safety.conflict.is_some());
    f.app.resolve_conflict(ConflictChoice::Cancel);
    f.app.apply_observations(vec![observation()]);
    assert!(f.app.safety.conflict.is_none());
    assert!(!f.app.save_document(id, false));
    f.app.resolve_conflict(ConflictChoice::Reload);
    f.app.apply_observations(vec![observation()]);
    assert!(f.app.safety.conflict.is_none());
}

#[test]
fn deleted_file_requires_review_before_recreation() {
    let (mut f, path, id) = opened();
    std::fs::remove_file(&path).unwrap();
    assert!(!f.app.save_document(id, false));
    assert!(f.app.safety.conflict.as_ref().unwrap().deleted);
    f.app.resolve_conflict(ConflictChoice::Keep);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
}

#[test]
fn unsaved_close_conflict_never_closes_until_resolution() {
    let (mut f, path, id) = opened();
    f.app.workspace.doc_mut().edit("draft".into(), 1);
    std::fs::write(&path, "external").unwrap();
    f.app.request_close(CloseAction::Tab(id));
    f.app.resolve_unsaved(UnsavedChoice::Save);
    assert!(f.app.pending.is_some());
    assert!(f.app.workspace.tabs.iter().any(|d| d.id == id));
    f.app.resolve_conflict(ConflictChoice::Keep);
    assert!(f.app.pending.is_none());
    assert!(!f.app.workspace.tabs.iter().any(|d| d.id == id));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "draft");
}

#[test]
fn recovery_roundtrip_keeps_order_active_drafts_and_original_baselines() {
    let (mut f, path, id) = opened();
    f.app.workspace.doc_mut().edit("नमस्ते 😀 draft".into(), 10);
    f.app.workspace.add("".into(), ViewMode::Split);
    f.app.workspace.doc_mut().edit("untitled draft".into(), 1);
    let folder = Some(f.dir.clone());
    let store = SessionStore::open(f.dir.clone()).unwrap();
    assert!(SessionStore::open(f.dir.clone()).is_err());
    let writer = markora::recovery::SessionWriter::start(store);
    writer
        .flush(Session::capture(&f.app.workspace, folder, &[]))
        .unwrap();
    drop(writer);
    std::fs::write(&path, "changed while closed").unwrap();
    let store = SessionStore::open(f.dir.clone()).unwrap();
    let session = store.load().unwrap().unwrap();
    assert_eq!(session.folder.as_ref(), Some(&f.dir));
    let (restored, errors) = session.restore(ViewMode::Source, true);
    assert!(errors.is_empty());
    assert_eq!(restored.tabs.len(), 3);
    assert_eq!(restored.doc().markdown, "untitled draft");
    assert_eq!(restored.doc().view_mode, ViewMode::Split);
    let doc = &restored.tabs[1];
    assert_eq!(doc.markdown, "नमस्ते 😀 draft");
    assert_eq!(doc.saved_markdown.as_deref(), Some("original"));
    assert!(doc.dirty());
    assert_ne!(doc.disk_version, FileVersion::read(&path).unwrap());
    assert_eq!(f.app.workspace.tabs[1].id, id);
}

#[test]
fn clean_tabs_use_current_disk_and_missing_files_recover_cached_text() {
    let (mut f, path, _) = opened();
    let session = Session::capture(&f.app.workspace, None, &[]);
    std::fs::write(&path, "new disk text").unwrap();
    let (restored, errors) = session.restore(ViewMode::Source, true);
    assert!(errors.is_empty());
    assert_eq!(restored.doc().markdown, "new disk text");
    assert!(!restored.doc().dirty());
    f.app.workspace = restored;
    let session = Session::capture(&f.app.workspace, None, &[]);
    std::fs::remove_file(path).unwrap();
    let (restored, errors) = session.restore(ViewMode::Source, true);
    assert_eq!(errors.len(), 1);
    assert_eq!(restored.doc().markdown, "new disk text");
    assert!(restored.doc().dirty());
}

#[test]
fn graceful_exit_keeps_saved_tabs_and_does_not_resurrect_discarded_drafts() {
    let (mut f, path, _) = opened();
    f.app.initialize_session();
    f.app
        .workspace
        .doc_mut()
        .edit("discard saved draft".into(), 1);
    f.app.workspace.add("".into(), ViewMode::Source);
    f.app
        .workspace
        .doc_mut()
        .edit("discard untitled draft".into(), 1);
    f.app.request_close(CloseAction::Exit);
    f.app.resolve_unsaved(UnsavedChoice::Discard);
    f.app.resolve_unsaved(UnsavedChoice::Discard);
    assert!(f.app.allow_close);
    f.app.safety.writer.take();
    let store = SessionStore::open(f.dir.clone()).unwrap();
    let (restored, errors) = store.load().unwrap().unwrap().restore(ViewMode::Live, true);
    assert!(errors.is_empty());
    assert!(restored.tabs.iter().all(|d| !d.dirty()));
    assert_eq!(
        restored
            .tabs
            .iter()
            .find(|d| d.path.as_ref() == Some(&path))
            .unwrap()
            .markdown,
        "original"
    );
    assert!(
        restored
            .tabs
            .iter()
            .all(|d| !d.markdown.contains("discard"))
    );
}

#[test]
fn corrupt_session_is_preserved_and_recovery_failure_does_not_close() {
    let mut f = Fixture::new();
    let session = f.dir.join("session.json");
    std::fs::write(&session, "broken recovery").unwrap();
    f.app.initialize_session();
    assert!(f.app.safety.writer.is_none());
    assert!(f.app.error);
    assert_eq!(
        std::fs::read_to_string(&session).unwrap(),
        "broken recovery"
    );
    std::fs::remove_file(&session).unwrap();
    f.app.initialize_session();
    std::fs::create_dir(&session).unwrap();
    f.app.request_close(CloseAction::Exit);
    assert!(!f.app.allow_close);
    assert!(f.app.error);
    assert!(session.is_dir());
}

#[test]
fn save_as_preserves_conflicting_original_and_rejects_an_open_destination() {
    let (mut f, original, id) = opened();
    f.app.workspace.doc_mut().edit("my copy".into(), 1);
    std::fs::write(&original, "external text").unwrap();
    assert!(!f.app.save_document(id, false));
    f.app.safety.conflict.take(); // The resolution dialog owns the subsequent Save As picker.
    let copy = f.dir.join("copy.md");
    assert!(f.app.save_to_path(id, copy.clone()));
    assert_eq!(std::fs::read_to_string(&original).unwrap(), "external text");
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "my copy");
    assert!(!f.app.workspace.doc().dirty());
    f.app.open_path(original.clone());
    let other = f.app.workspace.doc().id;
    f.app.workspace.doc_mut().edit("other draft".into(), 1);
    assert!(!f.app.save_to_path(other, copy.clone()));
    assert_eq!(std::fs::read_to_string(copy).unwrap(), "my copy");
    assert_eq!(f.app.workspace.doc().path.as_ref(), Some(&original));
    assert!(f.app.workspace.doc().dirty());
}

#[test]
fn reload_during_unsaved_close_finishes_without_overwriting_disk() {
    let (mut f, path, id) = opened();
    f.app.workspace.doc_mut().edit("draft".into(), 1);
    std::fs::write(&path, "external text").unwrap();
    f.app.request_close(CloseAction::Tab(id));
    f.app.resolve_unsaved(UnsavedChoice::Save);
    f.app.resolve_conflict(ConflictChoice::Reload);
    assert!(f.app.pending.is_none());
    assert!(!f.app.workspace.tabs.iter().any(|d| d.id == id));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "external text");
}

#[test]
fn cancel_after_discard_in_exit_flow_keeps_all_drafts_recoverable() {
    let (mut f, _, _) = opened();
    f.app.initialize_session();
    f.app.workspace.doc_mut().edit("saved file draft".into(), 1);
    f.app.workspace.add(String::new(), ViewMode::Live);
    f.app.workspace.doc_mut().edit("untitled draft".into(), 1);
    f.app.request_close(CloseAction::Exit);
    f.app.resolve_unsaved(UnsavedChoice::Discard);
    f.app.resolve_unsaved(UnsavedChoice::Cancel);
    assert!(!f.app.allow_close);
    assert!(f.app.flush_session());
    f.app.safety.writer.take();
    let store = SessionStore::open(f.dir.clone()).unwrap();
    let (restored, _) = store.load().unwrap().unwrap().restore(ViewMode::Live, true);
    assert_eq!(restored.tabs.iter().filter(|d| d.dirty()).count(), 2);
    assert_eq!(restored.doc().markdown, "untitled draft");
}
