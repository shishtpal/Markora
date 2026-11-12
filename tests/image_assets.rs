use markora::image_assets::{self, ImageInput};
use std::{fs, io::Cursor, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/image-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn document(&self) -> PathBuf {
        self.0.join("My notes # नमस्ते.md")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn bytes(format: image::ImageFormat) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        2,
        2,
        image::Rgb([20, 100, 180]),
    ));
    img.write_to(&mut buffer, format).unwrap();
    buffer.into_inner()
}
fn input(bytes: &[u8]) -> ImageInput {
    ImageInput {
        name: "Photo [नमस्ते].unknown".into(),
        bytes: bytes.to_vec(),
    }
}

#[test]
fn all_formats_preserve_bytes_deduplicate_and_never_overwrite_changed_assets() {
    let f = Fixture::new();
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
        image::ImageFormat::Gif,
    ] {
        let bytes = bytes(format);
        let first = image_assets::import(&f.document(), vec![input(&bytes)])
            .unwrap()
            .remove(0);
        let second = image_assets::import(&f.document(), vec![input(&bytes)])
            .unwrap()
            .remove(0);
        assert_eq!(first.relative_path, second.relative_path);
        assert_eq!(first.alt, "Photo [नमस्ते]");
        assert_eq!(fs::read(f.0.join(&first.relative_path)).unwrap(), bytes);
        fs::write(f.0.join(&first.relative_path), "external change").unwrap();
        assert!(image_assets::import(&f.document(), vec![input(&bytes)]).is_err());
        assert_eq!(
            fs::read_to_string(f.0.join(first.relative_path)).unwrap(),
            "external change"
        );
    }
}

#[test]
fn invalid_batch_creates_no_assets_and_limits_are_enforced() {
    let f = Fixture::new();
    let png = bytes(image::ImageFormat::Png);
    assert!(
        image_assets::import(&f.document(), vec![input(&png), input(b"not an image")]).is_err()
    );
    assert!(image_assets::import(&f.document(), vec![input(b"<svg></svg>")]).is_err());
    assert!(image_assets::import(&f.document(), vec![]).is_err());
    let mut bad = png.clone();
    bad.truncate(30);
    assert!(image_assets::import(&f.document(), vec![input(&bad)]).is_err());
    assert!(
        image_assets::import(
            &f.document(),
            vec![input(&vec![0; image_assets::MAX_IMAGE + 1])]
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}

#[test]
fn save_as_copies_referenced_assets_with_encoded_paths_and_refuses_conflicts() {
    let f = Fixture::new();
    let png = bytes(image::ImageFormat::Png);
    let image = image_assets::import(&f.document(), vec![input(&png)])
        .unwrap()
        .remove(0);
    let url = image.relative_path.replace(' ', "%20").replace('#', "%23");
    let markdown = format!("# Note\n\n![Photo]({url})\n\n[ordinary](missing.txt)");
    let destination = f.0.join("other");
    fs::create_dir(&destination).unwrap();
    image_assets::copy_references(&markdown, &f.document(), &destination.join("renamed.md"))
        .unwrap();
    let copied = destination.join(&image.relative_path);
    assert_eq!(fs::read(&copied).unwrap(), png);
    fs::write(&copied, "destination contents").unwrap();
    assert!(
        image_assets::copy_references(&markdown, &f.document(), &destination.join("renamed.md"))
            .is_err()
    );
    assert_eq!(fs::read_to_string(copied).unwrap(), "destination contents");
    fs::remove_file(f.0.join(image.relative_path)).unwrap();
    assert!(
        image_assets::copy_references(&markdown, &f.document(), &destination.join("renamed.md"))
            .is_err()
    );
}

#[test]
fn folder_policy_defaults_to_per_file_and_shared_images_are_reused_across_notes() {
    use markora::asset_policy::{AssetMode, AssetPolicy};
    let f = Fixture::new();
    let png = bytes(image::ImageFormat::Png);
    let policy = AssetPolicy {
        mode: AssetMode::Shared,
        custom_path: String::new(),
    };
    let first = image_assets::import_with_policy(&f.document(), vec![input(&png)], &policy)
        .unwrap()
        .remove(0);
    let second =
        image_assets::import_with_policy(&f.0.join("another.md"), vec![input(&png)], &policy)
            .unwrap()
            .remove(0);
    assert_eq!(first.source_path, second.source_path);
    assert!(first.relative_path.starts_with("data.assets/"));
    assert_eq!(fs::read_dir(f.0.join("data.assets")).unwrap().count(), 1);
    let old: markora::model::Prefs = serde_json::from_str("{}").unwrap();
    assert_eq!(old.asset_mode, AssetMode::File);
    let unknown: markora::model::Prefs = serde_json::from_str(r#"{"assetMode":"future"}"#).unwrap();
    assert_eq!(unknown.asset_mode, AssetMode::File);
}

#[test]
fn custom_external_folder_is_shared_and_only_registered_images_can_be_previewed() {
    use markora::{
        asset_policy::{AssetMode, AssetPolicy, encode_link},
        media::{MediaRoots, resolve_media_path},
    };
    let f = Fixture::new();
    let external = Fixture::new();
    let policy = AssetPolicy {
        mode: AssetMode::Custom,
        custom_path: external.0.to_string_lossy().into(),
    };
    let png = bytes(image::ImageFormat::Png);
    let imported = image_assets::import_with_policy(&f.document(), vec![input(&png)], &policy)
        .unwrap()
        .remove(0);
    assert!(imported.relative_path.starts_with("../"));
    let mut roots = MediaRoots::default();
    roots.docs.insert(1, f.0.clone());
    let raw_url = format!(
        "markora://localhost/doc/1/relative/{}",
        encode_link(&imported.relative_path).replace('/', "%2F")
    );
    assert!(resolve_media_path(&roots, &raw_url).is_none());
    roots.imported_images.insert(imported.source_path.clone());
    assert_eq!(
        resolve_media_path(&roots, &raw_url),
        Some(imported.source_path.clone())
    );
    let file_url = url::Url::from_file_path(&imported.source_path)
        .unwrap()
        .to_string();
    let route = format!(
        "markora://localhost/imported/{}",
        file_url
            .bytes()
            .map(|b| format!("%{b:02X}"))
            .collect::<String>()
    );
    assert_eq!(
        resolve_media_path(&roots, &route),
        Some(imported.source_path.clone())
    );
    let persisted = serde_json::to_string(&roots.imported_images).unwrap();
    roots.imported_images = serde_json::from_str(&persisted).unwrap();
    assert!(resolve_media_path(&roots, &raw_url).is_some());
    fs::write(external.0.join("secret.txt"), "private").unwrap();
    let secret = raw_url.replace(
        imported.source_path.file_name().unwrap().to_str().unwrap(),
        "secret.txt",
    );
    assert!(resolve_media_path(&roots, &secret).is_none());
}

#[test]
fn custom_relative_folder_and_save_as_rewrite_preserve_alt_and_share_absolute_assets() {
    use markora::asset_policy::{AssetMode, AssetPolicy, encode_link};
    let f = Fixture::new();
    let first_dir = f.0.join("notes");
    let next_dir = f.0.join("other");
    fs::create_dir_all(&first_dir).unwrap();
    fs::create_dir_all(&next_dir).unwrap();
    let first = first_dir.join("first.md");
    let next = next_dir.join("next.md");
    let png = bytes(image::ImageFormat::Png);
    let policy = AssetPolicy {
        mode: AssetMode::Custom,
        custom_path: "../images".into(),
    };
    let imported = image_assets::import_with_policy(&first, vec![input(&png)], &policy)
        .unwrap()
        .remove(0);
    let markdown = format!(
        "Before ![Keep this alt]({}) after",
        encode_link(&imported.relative_path)
    );
    let known = std::collections::HashSet::from([imported.source_path.clone()]);
    let policy = AssetPolicy {
        mode: AssetMode::Custom,
        custom_path: f.0.join("images").to_string_lossy().into(),
    };
    let (saved, images) =
        image_assets::relocate_references(&markdown, &first, &next, &policy, &known).unwrap();
    assert!(saved.contains("![Keep this alt]"));
    assert_eq!(images[0].source_path, imported.source_path);
    let (saved, images) = image_assets::relocate_references(
        &markdown,
        &first,
        &next,
        &AssetPolicy::default(),
        &known,
    )
    .unwrap();
    assert!(saved.contains("next.assets/"));
    assert!(
        images[0]
            .source_path
            .starts_with(next_dir.canonicalize().unwrap())
    );
    assert_eq!(fs::read(&images[0].source_path).unwrap(), png);
    assert!(
        AssetPolicy {
            mode: AssetMode::Custom,
            custom_path: " ".into()
        }
        .folder(&first)
        .is_err()
    );
}

#[cfg(windows)]
#[test]
fn another_drive_uses_a_file_url_instead_of_an_invalid_relative_path() {
    let url = markora::asset_policy::link_path(
        std::path::Path::new("D:\\notes"),
        std::path::Path::new("C:\\images\\photo name.png"),
    )
    .unwrap();
    assert_eq!(url, "file:///C:/images/photo%20name.png");
    let rendered = markora::markdown::render(&format!("![Image]({url})"));
    assert!(rendered.html.contains(&url));
    let link = markora::markdown::render(&format!("[File]({url})"));
    assert!(!link.html.contains(&url));
}

#[cfg(unix)]
#[test]
fn assets_directory_cannot_escape_through_a_symlink() {
    let f = Fixture::new();
    let outside = Fixture::new();
    std::os::unix::fs::symlink(&outside.0, f.0.join("My notes # नमस्ते.assets")).unwrap();
    assert!(
        image_assets::import(&f.document(), vec![input(&bytes(image::ImageFormat::Png))]).is_err()
    );
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
}
