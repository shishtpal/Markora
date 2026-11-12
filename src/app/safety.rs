//! Background external-change scans and the shared conflict dialog state machine.
use super::*;
use markora::{file_io::FileVersion, model::DocKind, recovery::SessionWriter};
use std::time::{Duration, Instant};

pub(super) struct FileConflict {
    pub token: u64,
    pub doc_id: u64,
    pub path: String,
    pub deleted: bool,
    pub version: FileVersion,
    pub saving: bool,
}

pub(crate) struct Observation {
    pub id: u64,
    pub path: PathBuf,
    pub baseline: FileVersion,
    pub result: Result<FileVersion, String>,
}

#[derive(PartialEq, Eq)]
pub(super) struct SessionKey {
    pub tabs: Vec<(
        u64,
        u64,
        Option<PathBuf>,
        markora::model::ViewMode,
        bool,
        FileVersion,
    )>,
    pub active: usize,
    pub folder: Option<PathBuf>,
}

pub(super) struct SafetyState {
    pub conflict_token: u64,
    pub conflict: Option<FileConflict>,
    pub writer: Option<SessionWriter>,
    pub discarded: Vec<u64>,
    pub session_key: Option<SessionKey>,
    pub changed_at: Instant,
    pub pending_since: Option<Instant>,
    pub last_scan: Instant,
    pub scanning: bool,
    pub last_error: Option<String>,
    pub dismissed: std::collections::HashMap<u64, FileVersion>,
}
impl Default for SafetyState {
    fn default() -> Self {
        Self {
            conflict_token: 0,
            conflict: None,
            writer: None,
            discarded: Vec::new(),
            session_key: None,
            changed_at: Instant::now(),
            pending_since: Some(Instant::now()),
            last_scan: Instant::now() - Duration::from_secs(2),
            scanning: false,
            last_error: None,
            dismissed: Default::default(),
        }
    }
}

impl Markora {
    pub(super) fn show_conflict(&mut self, id: u64, version: FileVersion, saving: bool) {
        let Some(doc) = self.workspace.tabs.iter().find(|d| d.id == id) else {
            return;
        };
        self.safety.conflict_token += 1;
        self.safety.conflict = Some(FileConflict {
            token: self.safety.conflict_token,
            doc_id: id,
            path: doc
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            deleted: version == FileVersion::Missing,
            version,
            saving,
        });
        self.needs_sync = true;
    }

    pub(super) fn apply_observations(&mut self, observations: Vec<Observation>) {
        self.safety.scanning = false;
        for observed in observations {
            let Some(doc) = self.workspace.tabs.iter().find(|d| d.id == observed.id) else {
                continue;
            };
            if doc.path.as_ref() != Some(&observed.path) || doc.disk_version != observed.baseline {
                continue;
            }
            match observed.result {
                Ok(version)
                    if version != doc.disk_version
                        && self.safety.conflict.is_none()
                        && self.safety.dismissed.get(&observed.id) != Some(&version) =>
                {
                    self.show_conflict(observed.id, version, false);
                }
                Err(error) if self.safety.last_error.as_ref() != Some(&error) => {
                    self.safety.last_error = Some(error.clone());
                    self.fail(format!("Could not check external changes: {error}"));
                }
                _ => {}
            }
        }
    }

    pub(super) fn scan_files(&mut self, ctx: &egui::Context) {
        if self.safety.scanning || self.safety.last_scan.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.safety.last_scan = Instant::now();
        let files: Vec<_> = self
            .workspace
            .tabs
            .iter()
            .filter(|d| d.kind == DocKind::Markdown)
            .filter_map(|d| d.path.clone().map(|path| (d.id, path, d.disk_version)))
            .collect();
        if files.is_empty() {
            return;
        }
        self.safety.scanning = true;
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let observations = files
                .into_iter()
                .map(|(id, path, baseline)| {
                    let result =
                        FileVersion::read(&path).map_err(|e| format!("{}: {e}", path.display()));
                    Observation {
                        id,
                        path,
                        baseline,
                        result,
                    }
                })
                .collect();
            let _ = tx.send(BgEvent::DiskScan(observations));
            ctx.request_repaint();
        });
    }
}
