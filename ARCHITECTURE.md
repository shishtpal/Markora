# Markora architecture

Markora is a local desktop editor. eframe owns the native window; wry embeds the
HTML interface as a WebView2 child. The Rust workspace owns documents and files.
The browser owns transient editing state, selections, menus, and dialogs.
On Linux, the same workspace and command handlers drive the native egui source
editor. `src/webview_fallback.rs` selects that interface without linking wry,
WebKit or GTK. The full web interface is currently Windows-only.

```mermaid
flowchart LR
    UI[UI source modules] -->|typed JSON + latest draft| IPC[WebView2 IPC]
    IPC -->|queue + request_repaint| App[Application dispatcher]
    App --> Workspace[Document workspace]
    App --> Storage[Local files and JSON settings]
    Workspace --> Parser[CommonMark renderer]
    App -->|state snapshot| UI
    Parser -->|HTML, blocks, outline| UI
```

## Rust modules

| File                              | Responsibility                                          |
| --------------------------------- | ------------------------------------------------------- |
| `src/main.rs`                     | Window configuration and startup                        |
| `src/app.rs`                      | Lifecycle, frame processing, close interception         |
| `src/app/bridge.rs`               | Deserialize requests, apply drafts, build snapshots     |
| `src/app/commands.rs`             | Native commands and unsaved-change state machine        |
| `src/app/files.rs`                | File/folder dialogs, open/save, recents                 |
| `src/app/export.rs`               | Exports and acknowledged print/PDF preparation          |
| `src/app/fallback.rs`             | Native source editor when WebView2 is unavailable       |
| `src/app/reading.rs`              | Transient reading state and native fullscreen lifecycle |
| `src/webview_keys.rs`             | F11 and PDF reading Escape through WebView2 accelerators |
| `src/model.rs`                    | Documents, revisions, dirty tracking, tabs, preferences |
| `src/markdown.rs`                 | CommonMark/GFM HTML, source block ranges, outline       |
| `src/protocol.rs`                 | Serde request and snapshot types                        |
| `src/storage.rs`                  | Settings persistence and filesystem operations          |
| `src/file_io.rs`                  | Same-directory atomic writes and content fingerprints   |
| `src/recovery.rs`                 | Versioned sessions, exclusive lock and ordered writer   |
| `src/app/session.rs`              | Startup restore, checkpoint scheduling and exit flush   |
| `src/app/safety.rs`               | Background file scans and external-change resolution    |
| `src/app/conflict.rs`             | Conflict choices, close continuation and native dialog  |
| `src/app/save.rs`                 | Save dialogs, checked writes and saved baselines        |
| `src/webview_host.rs`             | Child webview, IPC wakeup, bounds and message delivery  |
| `src/webview_fallback.rs`         | Non-Windows adapter selecting the native source editor  |
| `src/theme.rs`, `src/platform.rs` | Native styling/fonts and external URL opening           |

The library (`src/lib.rs`) exposes the model, parser, protocol and storage so
integration tests call production code. Host tests exercise the real command
dispatcher and close flow without creating a window.

## Frontend sources

Edit `assets/ui/shell.html`, the files in `assets/ui/src/`, and the stylesheets in
`assets/ui/styles/`. `assets/ui/index.html` and `tailwind/app.built.css` are generated.

| Module                           | Responsibility                                                       |
| -------------------------------- | -------------------------------------------------------------------- |
| `main.js`                        | Connect the frontend modules                                         |
| `bridge.js`                      | Native transport, startup handshake, draft attachment                |
| `command-list.js`, `commands.js` | Shared command catalog, menus, shortcuts, dispatch                   |
| `editor.js`                      | Document draft, revisions, source/split editing, formatting, history |
| `live-editor.js`                 | Rendered blocks with an active Markdown textarea                     |
| `layout.js`                      | Responsive sizing, pointer/keyboard resizers and saved widths        |
| `reading.js`                     | Read-only preview presentation and editor focus/selection/scroll restore |
| `chrome.js`                      | Tabs, file lists, recents, outline, status and themes                |
| `dialogs.js`, `find.js`          | Modal UI, palette, settings and document search                      |
| `search-engine.js`               | Unicode-aware matching, bounded replacement and regex captures       |
| `search-worker.js`               | Cancellable worker execution with search timeouts                    |
| `conflicts.js`                   | External file-change dialog and resolution actions                   |
| `dom.js`                         | Small shared DOM helpers                                             |

## Data flow and invariants

1. Every user action carries its document ID, latest Markdown and revision. Rust
   applies that draft before save, tab activation, close, or command execution.
2. Every IPC arrival requests an egui repaint. Commands work while the parent
   window is otherwise idle. There is no continuous frame polling requirement.
3. The page retries its readiness handshake until it receives a state snapshot.
   Host messages are sent only after that handshake, when the receiver exists.
4. Revisions prevent stale drafts from replacing newer content. Frontend echoes
   do not replace an active textarea or reset its cursor.
5. One Rust CommonMark parser generates preview HTML, live blocks, and outline.
   Block offsets sent to JavaScript use UTF-16 positions, including emoji.
6. Live editing splices the active block's Markdown into its original source
   range. It does not reconstruct Markdown from rendered DOM.
7. Closing a tab checks that tab. Closing all tabs or exiting considers every
   dirty document and offers Save, Discard, or Cancel inside the visible UI.
8. Grid columns have explicit placement, so hiding a sidebar frees its width.
   No editor view has a fixed maximum document width.
9. Find and replace searches the Markdown model in a Blob worker, bundled fully
   offline. Results are accepted only for the originating document, revision,
   query and options. Changing the query or closing the bar cancels outstanding
   work. Regex execution has a 1.5-second timeout; truncated match sets cannot
   be replaced in bulk. A replacement updates the document once through the
   editor's normal revision/history/IPC flow, making Replace All one undo step.

F11 goes through the shared command catalog. WebView2 native accelerators forward
F11 (and Escape while reading a PDF) to the page after their COM callback returns;
the page sends a normal command with its latest draft. Rust owns transient reading
state, prepares deferred Source rendering and requests native fullscreen through
egui. State snapshots drive `reading.js`, which reuses the existing preview and
keeps the Live/Source/Split mode and saved preferences intact. Winit restores native
window placement on fullscreen exit. The frontend preserves editing focus,
selection and scroll; find navigates rendered source-block anchors while mutation
commands and pending replacements are disabled. The mode is not persisted.

## Build and persistence

`node tools/build-ui.cjs` compiles Tailwind, bundles the named-import JavaScript
modules, and validates their dependency order. Rust embeds the HTML and fonts,
serving them through the local asset protocol. Fonts load on demand and HTML
responses bypass the browser cache so updates are visible after a restart.
There is no runtime Node or web-server dependency.

File reads and preparation run on background threads, overlapping WebView2
startup for command-line files. Large Source documents defer HTML generation;
Live inserts blocks in frame batches. State snapshots omit unchanged document
content and reuse the browser's current draft. Printing explicitly waits for
the browser to prepare the full preview before invoking the native print API.

`cargo build` embeds that generated asset. Debug builds read the on-disk bundle
on startup; release builds use the embedded copy. Rebuild the UI before Cargo
when changing frontend sources. The watch command rebuilds sources automatically;
restart the app to load a new bundle.

Settings and recents use a `.markora/` directory alongside the `markora`
executable, with an AppData fallback when that directory cannot be created.
`MARKORA_DATA_DIR` overrides settings storage for isolated runs. Documents are
saved to their selected paths; they are not stored inside the preferences files.

`session.json` separately stores Markdown drafts, their last saved baseline and
disk content fingerprint, plus tab order, selection, view modes and folder.
Clean files reopen from disk; dirty drafts retain their original fingerprint so
offline changes still require review. An exclusive `session.lock` file lock
prevents multiple instances from overwriting the same recovery session. Corrupt
or unknown-version sessions disable recovery writes and retain the original.
One worker serializes checkpoints through a bounded queue; an explicit exit
waits for its final checkpoint. Discarded untitled drafts are omitted, and
discarded file-backed drafts restore from disk. Cancel leaves drafts intact.

The event loop requests a repaint every 500 ms to schedule recovery and file
monitoring. A debounced checkpoint runs after 750 ms of inactivity, with a
two-second maximum scheduling interval while typing. File content fingerprints
are scanned off the UI thread every two seconds; results with an outdated path
or baseline are ignored. Saves compare the expected fingerprint both before
writing the temporary file and immediately before replacement. This is
optimistic conflict detection, not an exclusive lock on other applications.

## Verification

- `cargo test`: production model, parser, storage, protocol and host regressions.
- `cargo clippy --all-targets -- -D warnings`: Rust checks.
- `node tools/verify-ui.cjs`: bundle syntax, IDs, embedded fonts, 300-line module limit.
- `node tools/smoke-native.cjs`: actual Rust + WebView2 menus, shortcuts, palette,
  editing, resizing, fullscreen reading and window restoration, width, unsaved
  changes and file save. Requires Windows.
- `node tools/smoke-safety.cjs`: external conflicts, Live reload, forced shutdown,
  restored tabs/drafts and graceful-exit discard in real WebView2 processes.
- `node --test tools/test-search.cjs`: production search semantics, Unicode
  boundaries/offsets, capture substitutions and result limits.
- `node tools/smoke-find.cjs`: real WebView2 find/replace, navigation, undo/save,
  regex timeouts/cancellation, stale-result rejection and Live/Source/Split.

The native suite creates fixtures, settings and WebView2 data under
`target/ui-smoke/<run>/` and never uses the normal preferences directory. It also
saves screenshots and runtime diagnostics there. Its DevTools port exists only
for the lifetime of its isolated test process.
