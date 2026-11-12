//! Reading transitions use the production IPC dispatcher and deferred renderer.
use super::tests::Fixture;
use markora::model::ViewMode;

#[test]
fn reading_uses_latest_deferred_source_draft_without_changing_preferences() {
    let mut fixture = Fixture::new();
    fixture
        .app
        .workspace
        .doc_mut()
        .set_view_mode(ViewMode::Source);
    let prefs = serde_json::to_value(&fixture.app.prefs).unwrap();
    let markdown = format!("# Latest draft\n\n{}", "A paragraph.\n\n".repeat(15_000));
    fixture.request(serde_json::json!({
        "type":"command", "command":"toggle-reading",
        "draft":{"docId":1,"markdown":markdown,"revision":7}
    }));
    let doc = fixture.app.workspace.doc();
    assert!(fixture.app.reading.active);
    assert_eq!(doc.markdown, markdown);
    assert_eq!(doc.revision, 7);
    assert_eq!(doc.view_mode, ViewMode::Source);
    assert!(doc.rendered_full);
    assert_eq!(doc.rendered_revision, 7);
    assert!(doc.rendered.html.contains("Latest draft"));
    fixture.request(serde_json::json!({"type":"command","command":"exit-reading"}));
    assert!(!fixture.app.reading.active);
    assert_eq!(serde_json::to_value(&fixture.app.prefs).unwrap(), prefs);
    assert!(!fixture.dir.join("prefs.json").exists());
}

#[test]
fn reading_navigation_and_reload_prepare_new_source_content() {
    let mut fixture = Fixture::new();
    fixture.app.set_reading(true);
    let markdown = "Another paragraph.\n\n".repeat(15_000);
    fixture.app.workspace.add(markdown, ViewMode::Source);
    assert!(!fixture.app.workspace.doc().rendered_full);
    let ctx = eframe::egui::Context::default();
    let mut output = ctx.run_ui(Default::default(), |ui| fixture.app.reading_frame(ui.ctx()));
    output.textures_delta.clear();
    assert!(fixture.app.workspace.doc().rendered_full);
    assert!(fixture.app.reading.active);
    fixture.app.run_command("toggle-reading");
    assert!(!fixture.app.reading.active);
    fixture.app.run_command("exit-reading");
    assert!(!fixture.app.reading.active);
}

#[test]
fn reading_restores_an_existing_fullscreen_window() {
    let mut fixture = Fixture::new();
    let ctx = eframe::egui::Context::default();
    fixture.app.egui_ctx = Some(ctx.clone());
    let mut input = eframe::egui::RawInput::default();
    input
        .viewports
        .get_mut(&eframe::egui::ViewportId::ROOT)
        .unwrap()
        .fullscreen = Some(true);
    ctx.begin_pass(input);
    fixture.app.set_reading(true);
    fixture.app.set_reading(false);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    let commands = &output.viewport_output[&eframe::egui::ViewportId::ROOT].commands;
    assert_eq!(
        commands
            .iter()
            .filter(|cmd| matches!(cmd, eframe::egui::ViewportCommand::Fullscreen(true)))
            .count(),
        2
    );
    assert!(
        !commands
            .iter()
            .any(|cmd| matches!(cmd, eframe::egui::ViewportCommand::Fullscreen(false)))
    );
}

#[test]
fn rapid_reading_reentry_does_not_capture_a_pending_fullscreen_exit() {
    let mut fixture = Fixture::new();
    let ctx = eframe::egui::Context::default();
    fixture.app.egui_ctx = Some(ctx.clone());
    ctx.begin_pass(Default::default());
    fixture.app.set_reading(true);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    let mut input = eframe::egui::RawInput::default();
    input
        .viewports
        .get_mut(&eframe::egui::ViewportId::ROOT)
        .unwrap()
        .fullscreen = Some(true);
    ctx.begin_pass(input);
    fixture.app.set_reading(false);
    fixture.app.set_reading(true);
    fixture.app.set_reading(false);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    assert!(matches!(
        output.viewport_output[&eframe::egui::ViewportId::ROOT]
            .commands
            .last(),
        Some(eframe::egui::ViewportCommand::Fullscreen(false))
    ));
}
