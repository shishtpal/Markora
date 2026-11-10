//! Decode requests, apply the draft first, then dispatch exactly one action.
use super::*;
use markora::protocol::{Action, DocumentView, Request, Snapshot, TabView};

#[derive(serde::Serialize)]
struct StateMessage<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    snapshot: Snapshot<'a>,
}

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
                if self
                    .workspace
                    .tabs
                    .iter()
                    .any(|d| d.id == draft.doc_id && d.revision == draft.revision)
                {
                    self.client_known_markdown = Some((draft.doc_id, draft.revision));
                }
            }
            self.needs_sync = true;
            match request.action {
                Action::Ready => {
                    self.page_ready = true;
                    self.synced_doc = None;
                    self.client_known_markdown = None;
                }
                Action::Edit => {}
                Action::PrintReady { token } => self.finish_print(token),
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
                        self.workspace.doc_mut().set_view_mode(view);
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
                .filter_map(|d| {
                    d.path
                        .as_ref()
                        .and_then(|p| p.parent())
                        .map(|dir| (d.id, dir.to_path_buf()))
                })
                .collect();
        }
        let Some(host) = &self.webview else {
            return;
        };
        let doc = self.workspace.doc();
        let current_state = SyncedDocState {
            id: doc.id,
            revision: doc.revision,
            view_mode: doc.view_mode,
            rendered_revision: doc.rendered_revision,
            rendered_full: doc.rendered_full,
        };
        let document = if self.synced_doc == Some(current_state) {
            None
        } else {
            let need_markdown = self.synced_doc.is_none_or(|s| s.id != doc.id)
                || self.client_known_markdown != Some((doc.id, doc.revision));
            let need_rendered = self.synced_doc.is_none_or(|s| {
                s.id != doc.id
                    || s.rendered_revision != doc.rendered_revision
                    || s.rendered_full != doc.rendered_full
            });
            Some(DocumentView {
                id: doc.id,
                markdown: need_markdown.then_some(doc.markdown.as_str()),
                revision: doc.revision,
                view_mode: doc.view_mode,
                rendered: need_rendered.then_some(&doc.rendered),
                word_count: doc.word_count,
                char_count: doc.char_count,
            })
        };
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
            document,
            active_doc_id: doc.id,
            view_mode: doc.view_mode,
            word_count: doc.word_count,
            char_count: doc.char_count,
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
        let result = host.send(&StateMessage {
            kind: "state",
            snapshot,
        });
        match result {
            Ok(()) => {
                self.needs_sync = false;
                self.synced_doc = Some(current_state);
                self.client_known_markdown = Some((doc.id, doc.revision));
            }
            Err(error) => self.fail(format!("UI synchronization failed: {error}")),
        }
    }
}
