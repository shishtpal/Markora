//! Versioned session snapshots, exclusive ownership and a single ordered disk writer.
use crate::{
    file_io::FileVersion,
    model::{DocKind, Document, ViewMode, Workspace},
    storage::{self, Storage},
};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf, sync::mpsc};

#[derive(Deserialize, Serialize)]
pub struct Session {
    version: u32,
    active: usize,
    pub folder: Option<PathBuf>,
    tabs: Vec<SavedTab>,
}

#[derive(Deserialize, Serialize)]
struct SavedTab {
    kind: DocKind,
    path: Option<PathBuf>,
    view: ViewMode,
    markdown: String,
    baseline: Option<String>,
    disk_version: FileVersion,
}

impl Session {
    pub fn capture(workspace: &Workspace, folder: Option<PathBuf>, discarded: &[u64]) -> Self {
        let mut tabs = Vec::new();
        let mut active = 0;
        for (index, doc) in workspace.tabs.iter().enumerate() {
            let discard = discarded.contains(&doc.id);
            if discard && doc.path.is_none() {
                continue;
            }
            if index == workspace.active {
                active = tabs.len();
            }
            tabs.push(SavedTab {
                kind: doc.kind,
                path: doc.path.clone(),
                view: doc.view_mode,
                markdown: if discard {
                    doc.saved_markdown.as_ref().unwrap_or(&doc.markdown).clone()
                } else {
                    doc.markdown.clone()
                },
                baseline: if discard {
                    None
                } else {
                    doc.saved_markdown.clone()
                },
                disk_version: doc.disk_version,
            });
        }
        Self {
            version: 1,
            active,
            folder,
            tabs,
        }
    }

    pub fn restore(self, default_view: ViewMode, parse_full_pdf: bool) -> (Workspace, Vec<String>) {
        let mut workspace = Workspace::new(String::new(), default_view);
        workspace.tabs.clear();
        let mut errors = Vec::new();
        let mut active = 0;
        for (index, tab) in self.tabs.into_iter().enumerate() {
            let id = workspace.allocate_id();
            let mut doc = if let Some(path) = &tab.path {
                if tab.baseline.is_none() {
                    match storage::prepare_file(path, tab.view, parse_full_pdf) {
                        Ok(prepared) => {
                            Document::from_prepared(id, path.clone(), tab.view, prepared)
                        }
                        Err(error) => {
                            errors.push(format!("{}: {error}", path.display()));
                            if tab.kind == DocKind::Pdf {
                                continue;
                            }
                            let mut doc = Document::new(id, tab.markdown.clone(), tab.view);
                            doc.saved_markdown = Some(String::new());
                            doc.disk_version = tab.disk_version;
                            doc.path = Some(path.clone());
                            doc
                        }
                    }
                } else {
                    let mut doc = Document::new(id, tab.markdown, tab.view);
                    doc.path = Some(path.clone());
                    doc.disk_version = tab.disk_version;
                    doc.saved_markdown = tab.baseline;
                    doc
                }
            } else {
                let mut doc = Document::new(id, tab.markdown, tab.view);
                doc.saved_markdown = tab.baseline;
                doc
            };
            // A restored document starts a fresh frontend revision history.
            doc.revision = 0;
            if index == self.active {
                active = workspace.tabs.len();
            }
            workspace.tabs.push(doc);
        }
        if workspace.tabs.is_empty() {
            workspace.add(String::new(), default_view);
        }
        workspace.active = active.min(workspace.tabs.len() - 1);
        (workspace, errors)
    }
}

pub struct SessionStore {
    storage: Storage,
    _lock: fs::File,
}

impl SessionStore {
    pub fn open(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("session.lock"))?;
        lock.try_lock().map_err(|e| {
            io::Error::other(format!(
                "Session recovery is already in use or unavailable: {e}"
            ))
        })?;
        Ok(Self {
            storage: Storage { dir },
            _lock: lock,
        })
    }
    pub fn load(&self) -> io::Result<Option<Session>> {
        let file = match fs::File::open(self.storage.dir.join("session.json")) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let session: Session = serde_json::from_reader(file).map_err(io::Error::other)?;
        if session.version != 1 {
            return Err(io::Error::other(
                "Unsupported recovery version; session.json was preserved",
            ));
        }
        Ok(Some(session))
    }
    fn save(&self, session: &Session) -> io::Result<()> {
        self.storage.write("session.json", session)
    }
}

type Checkpoint = (Session, Option<mpsc::Sender<Result<(), String>>>);
pub struct SessionWriter {
    tx: Option<mpsc::SyncSender<Checkpoint>>,
    pub results: mpsc::Receiver<Result<(), String>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl SessionWriter {
    pub fn start(store: SessionStore) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Checkpoint>(1);
        let (result_tx, results) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            while let Ok((session, ack)) = rx.recv() {
                let result = store.save(&session).map_err(|e| e.to_string());
                if let Some(ack) = ack {
                    let _ = ack.send(result);
                } else {
                    let _ = result_tx.send(result);
                }
            }
        });
        Self {
            tx: Some(tx),
            results,
            thread: Some(thread),
        }
    }
    pub fn schedule(&self, session: Session) -> bool {
        self.tx.as_ref().unwrap().try_send((session, None)).is_ok()
    }
    pub fn flush(&self, session: Session) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .unwrap()
            .send((session, Some(tx)))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
}
impl Drop for SessionWriter {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
