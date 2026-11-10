//! Reproduce asynchronous completion ordering without relying on thread timing.
use super::{tests::Fixture, *};
use markora::model::{Document, ViewMode};

#[test]
fn late_placeholder_draft_cannot_overwrite_opened_file() {
    let mut fixture = Fixture::new();
    fixture.app.workspace = Workspace::new(String::new(), ViewMode::Live);
    fixture
        .app
        .bg_tx
        .send(BgEvent::FileLoaded {
            path: fixture.dir.join("loaded.md"),
            view_mode: ViewMode::Live,
            result: Ok(Document::prepare("# Loaded".into(), ViewMode::Live)),
        })
        .unwrap();
    fixture.app.drain_bg();
    assert_ne!(fixture.app.workspace.doc().id, 1);
    fixture.request(serde_json::json!({"type":"edit","draft":{
        "docId":1,"markdown":"","revision":0
    }}));
    assert_eq!(fixture.app.workspace.doc().markdown, "# Loaded");
    assert!(!fixture.app.workspace.doc().dirty());
}

#[test]
fn prepared_file_keeps_its_requested_view_mode() {
    let mut fixture = Fixture::new();
    let markdown = "# Heading\n\n".repeat(6000);
    let prepared = Document::prepare(markdown, ViewMode::Source);
    assert!(!prepared.rendered_full);
    fixture.app.prefs.view_mode = ViewMode::Live;
    fixture
        .app
        .bg_tx
        .send(BgEvent::FileLoaded {
            path: fixture.dir.join("large.md"),
            view_mode: ViewMode::Source,
            result: Ok(prepared),
        })
        .unwrap();
    fixture.app.drain_bg();
    assert_eq!(fixture.app.workspace.doc().view_mode, ViewMode::Source);
    fixture.app.run_command("view-live");
    assert!(fixture.app.workspace.doc().rendered_full);
    assert_eq!(fixture.app.workspace.doc().rendered.blocks.len(), 6000);
}

#[test]
fn outdated_folder_results_and_errors_are_ignored() {
    let mut fixture = Fixture::new();
    fixture.app.folder_generation = 2;
    let latest = fixture.dir.join("latest");
    for (path, generation, result) in [
        (latest.clone(), 2, Ok(Vec::new())),
        (fixture.dir.join("old"), 1, Ok(Vec::new())),
        (fixture.dir.join("failed"), 1, Err("late error".into())),
    ] {
        fixture
            .app
            .bg_tx
            .send(BgEvent::FolderLoaded {
                path,
                generation,
                result,
            })
            .unwrap();
    }
    fixture.app.drain_bg();
    assert_eq!(fixture.app.folder, Some(latest));
    assert!(!fixture.app.error);
}

#[test]
fn print_acknowledgement_rejects_a_changed_document() {
    let mut fixture = Fixture::new();
    fixture.app.pending_print = Some((1, 1, 0, None));
    fixture.request(serde_json::json!({"type":"printReady","token":1,"draft":{
        "docId":1,"markdown":"new text","revision":1
    }}));
    assert!(fixture.app.error);
    assert!(fixture.app.pending_print.is_none());
    assert_eq!(fixture.app.workspace.doc().markdown, "new text");
}
