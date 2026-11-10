//! Decode requests, apply the draft first, then dispatch exactly one action.
use super::*;
use markora::protocol::{Action, DocumentView, Request, Snapshot, TabView};

impl Markora {
    pub(super) fn drain_bridge(&mut self) {
        let messages: Vec<_> = self
            .queue
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();
        for message in messages {
            let request = match serde_json::from_str::<Request>(&message) {
                Ok(request) => request,
                Err(error) => {
                    self.fail(format!("Invalid UI request: {error}"));
                    continue;
                }
            };
            if let Some(draft) = request.draft {
                self.workspace
                    .edit(draft.doc_id, draft.markdown, draft.revision);
            }
            self.needs_sync = true;
            match request.action {
                Action::Ready => self.page_ready = true,
                Action::Edit => {}
                Action::Command { command } => self.run_command(&command),
                Action::OpenPath { path } => self.open_path(path.into()),
                Action::OpenLink { url } => {
                    if let Err(error) = crate::platform::open_web_link(&url) {
                        self.fail(error);
                    }
                }
                Action::ActivateTab { id } => self.workspace.activate(id),
                Action::CloseTab { id } => self.request_close(CloseAction::Tab(id)),
                Action::Preferences { patch } => {
                    let view = patch.view_mode;
                    patch.apply(&mut self.prefs);
                    if let Some(view) = view {
                        self.workspace.doc_mut().view_mode = view;
                    }
                    self.persist_prefs();
                }
                Action::ResolveUnsaved { choice } => self.resolve_unsaved(choice),
                Action::Export { format, html_style } => {
                    self.export_document(format, html_style);
                }
            }
        }
    }

    pub(super) fn sync_webview(&mut self) {
        if !self.page_ready || !self.needs_sync {
            return;
        }
        if let Ok(mut roots) = self.media_roots.write() {
            roots.folder = self.folder.clone();
            roots.docs = self
                .workspace
                .tabs
                .iter()
                .filter_map(|d| d.path.as_ref().and_then(|p| p.parent()).map(|dir| (d.id, dir.to_path_buf())))
                .collect();
        }
        let Some(host) = &self.webview else {
            return;
        };
        let doc = self.workspace.doc();
        let snapshot = Snapshot {
            tabs: self
                .workspace
                .tabs
                .iter()
                .map(|d| TabView {
                    id: d.id,
                    title: d.title(),
                    dirty: d.dirty(),
                    path: d.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                })
                .collect(),
            document: DocumentView {
                id: doc.id,
                markdown: &doc.markdown,
                revision: doc.revision,
                view_mode: doc.view_mode,
                rendered: &doc.rendered,
            },
            prefs: &self.prefs,
            folder: self
                .folder
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            entries: &self.entries,
            recents: &self.recents,
            status: &self.status,
            error: self.error,
            pending: self
                .pending
                .as_ref()
                .and_then(|p| p.documents.front())
                .and_then(|id| self.workspace.tabs.iter().find(|d| d.id == *id))
                .map(|d| d.title()),
        };
        let result = host.send(&serde_json::json!({"type":"state", "snapshot": snapshot}));
        match result {
            Ok(()) => self.needs_sync = false,
            Err(error) => self.fail(format!("UI synchronization failed: {error}")),
        }
    }
}
