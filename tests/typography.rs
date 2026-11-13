use markora::{model::Prefs, protocol::PrefsPatch};

#[test]
fn older_preferences_receive_typography_defaults() {
    let prefs: Prefs = serde_json::from_str(r#"{"fontScale":1.2,"mdTheme":"paper"}"#).unwrap();
    assert_eq!(prefs.font_family, "theme");
    assert_eq!(prefs.font_size, 16.0);
    assert_eq!(prefs.font_scale, 1.2);
    assert_eq!(prefs.md_theme, "paper");
}

#[test]
fn typography_patches_validate_font_ids_and_size_bounds() {
    let mut prefs = Prefs::default();
    let patch: PrefsPatch =
        serde_json::from_str(r#"{"fontFamily":"georgia","fontSize":24}"#).unwrap();
    patch.apply(&mut prefs);
    assert_eq!(prefs.font_family, "georgia");
    assert_eq!(prefs.font_size, 24.0);
    let patch: PrefsPatch =
        serde_json::from_str(r#"{"fontFamily":"invalid","fontSize":100}"#).unwrap();
    patch.apply(&mut prefs);
    assert_eq!(prefs.font_family, "theme");
    assert_eq!(prefs.font_size, 32.0);
    prefs.font_size = f32::NAN;
    prefs.validate();
    assert_eq!(prefs.font_size, 16.0);
    prefs.font_size = 1.0;
    prefs.validate();
    assert_eq!(prefs.font_size, 12.0);
}
