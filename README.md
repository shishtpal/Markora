# Markora

A local Markdown editor with a Rust/egui desktop host, a WebView2 interface on
Windows, and a native source editor on Linux.
The editor and its assets are original; the project is inspired by Typora's
single-document editing concept.

## Download and platform support

After a version tag is published, download the Windows ZIP or Linux tarball from
this repository's **Releases** page. Extract the archive and run `markora.exe`
on Windows or `./markora` on Linux. Archives include licenses and checksums.

| Platform       | Interface                                                    | Requirements                                                              |
| -------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------- |
| Windows x86_64 | Live, Source and Split                                       | WebView2 Evergreen Runtime and Microsoft Visual C++ v14 x64 runtime       |
| Linux x86_64   | Native source editor, tabs, files, outline and save controls | X11/Wayland desktop, OpenGL/EGL, glibc 2.35+; XDG portal for file dialogs |

Linux does not yet have the web interface or its Live/Split editing modes.
Windows packages are portable and unsigned. Node and Rust are build tools only.
Windows runtime downloads: [WebView2 Evergreen](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
and [Visual C++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist).

## Editing (Windows web interface)

- **Live:** click a rendered block to edit its Markdown. Escape renders it again;
  Ctrl+Enter starts a paragraph after the block.
- **Source:** edit the complete Markdown document.
- **Split:** edit source beside the rendered preview; drag the divider to resize.
- All three modes fill the available document workspace width.
- Files and Outline sidebars have pointer and keyboard resizers. Their widths
  persist; double-click a divider or use View → Reset sidebar widths to reset.
- Tabs, recents, folder browsing, outline navigation, formatting, undo/redo,
  find, command palette, focus mode, settings, light/dark and seven Markdown themes.
- **GitHub theme:** GitHub-style typography, heading rules, code, tables and quotes
  in light/dark modes. Select Settings → Markdown theme → GitHub; it also appears
  in the theme cycle. Palette reference: [github-markdown-css](https://github.com/sindresorhus/github-markdown-css).
- CommonMark/GFM rendering supports headings, lists, quotes, fenced code, links,
  images, tables, strikethrough and task lists. Raw HTML is displayed as text.

The Live editor keeps the active block as Markdown source. Its edits, source
editing, previews and file saves all use the same document model. There is no
DOM-to-Markdown conversion step.

## Fullscreen reading

Press **F11**, or choose **View → Fullscreen reading view**, to read the current
document fullscreen. Markdown uses a rendered, read-only preview; PDF tabs keep
their existing viewer. Menus, tabs, formatting controls, sidebars and the status
bar are hidden. Scrolling, copying, links and **Ctrl+F** remain available.

Press **F11**, **Escape**, or **Exit reading** to return to editing. Escape closes
an open dialog or find bar first. The previous editor mode, selection, scroll
position and window placement are restored. Reading does not change saved focus
or sidebar preferences and is not restored after restarting the app.

On Linux and the native fallback interface, F11 shows the existing source or PDF
view fullscreen with editing disabled; rendered Markdown reading requires the
Windows web interface.

## Find and replace (Windows web interface)

Use **Ctrl+F** to find or **Ctrl+H** for find and replace; both are also in the
Edit menu and command palette. **Aa** matches case, **Word** matches whole words,
and **.*** enables regular expressions. Options can be combined. Whole-word
boundaries recognize Unicode letters, combining marks, numbers and connectors
such as underscores. Search covers the Markdown source, including its syntax.

- **F3 / Shift+F3** or **Enter / Shift+Enter** in the find field navigate and wrap
  around matches. Escape closes the bar.
- **Replace** changes the selected match, or the first match if none is selected,
  then selects the next match. **Replace all** changes all matches in the active
  document as one undo step. An empty replacement deletes matching text.
- In the replacement field, **Enter** replaces one match and **Ctrl+Enter**
  replaces all. Literal mode treats replacement text literally. Regex mode
  supports numbered and named captures (`$1`, `$<name>`), `$&` for the match,
  and `$$` for a literal dollar sign. Patterns use JavaScript Unicode regex
  syntax with multiline anchors; enter the pattern without `/…/` delimiters.
- Replacements work in Live, Source and Split, update previews and dirty state,
  and use the normal undo, save and recovery flows. PDF tabs remain read-only.
- Invalid patterns display an error. Searches run outside the UI thread and
  cancel after 1.5 seconds. At most 100,000 matches are displayed; refine a
  limited search before replacing all. Replacement results are limited to
  32 million characters. Cancelled, failed or stale results do not edit a document.

## Image paste and drop

Paste a clipboard screenshot, drop image files into the document workspace, or
choose **Format → Insert image…**. Windows Live, Source and Split insert ordinary
Markdown image links. Normal text paste and Markdown/PDF file drops keep their
existing behavior. Drop images and documents separately.

In **Settings → Image asset folders**, choose:

- **Per file:** `<note-name>.assets/` beside each note (the default).
- **Shared:** one `data.assets/` folder for all notes in the same directory.
- **Custom:** an absolute folder shared by all notes, or a path relative to each
  note's directory. Type a path or use **Browse…**. Paths on the same drive use
  relative links; another drive uses `file:` URLs.

The setting is saved and applies to new imports; existing links keep working.
Files use content-based names. Identical bytes reuse the existing image; unrelated files are never
overwritten. The original PNG, JPEG, WebP or GIF file bytes are preserved.
Imports accept up to 10 images, 20 MiB each, 50 MiB per batch and 32 megapixels.
Other formats must first be converted to a supported format.

An untitled note asks for a save location before importing anything. Cancel
leaves the draft and assets untouched. Imported links are dirty edits until you
save the note, and each gesture is one Undo step. Undo retains the image files
so Redo and draft recovery continue to work.

Paste uses the current text selection. Dropping onto a rendered Live block
inserts an image paragraph after it; Source and Split use the source selection.
If you edit, switch tabs or enter reading view during an import, completed
images remain on disk and **Insert at cursor** lets you insert them safely after
returning to the original note. Images cannot be inserted into PDFs or reading
view. Closing or dismissing an import does not automatically delete its assets.

Save As copies referenced Markora images into the selected destination policy
and updates their links. An absolute custom folder is reused without duplication.
Copy errors stop the Markdown save and retain
the original document. Moving a note manually requires moving its assets too.
The native source interface supports image file drops at its selection; native
bitmap clipboard import is not included.
Its **Image folders** button exposes the same folder preferences.

## Saving, recovery and external changes

- Markdown, settings and recovery writes use a synced temporary file beside the
  destination, followed by replacement. A failed write does not truncate the
  existing file. Read-only destinations remain protected.
- Open tabs, their order, active tab, view modes, folder and Markdown drafts are
  checkpointed to `.markora/session.json`. Checkpoints run after a short idle
  period, or about every two seconds during continuous typing. Recovery does
  not automatically save drafts into their document files.
- Restarting restores saved tabs and unsaved drafts, including untitled notes.
  Clean saved tabs reopen the current disk contents. Missing Markdown files
  retain their cached text as recoverable drafts. Explicitly discarded drafts
  are excluded from the session on a successful normal exit.
- Open Markdown files are checked in the background about every two seconds,
  and again before saving. External edits or deletion offer **Reload**, **Keep
  Mine**, **Save As** and **Later**. Reload replaces the draft; Keep Mine
  acknowledges the reviewed disk version and preserves your draft. If a save
  triggered the dialog, Keep Mine resumes that save. Later postpones the prompt
  for that disk version; saving still requires review.
- One running instance owns recovery for each settings directory. Additional
  instances can edit files but report that session recovery is unavailable.
  A malformed recovery file is preserved instead of silently overwritten.

Recovery covers the last completed checkpoint; edits made immediately before a
forced shutdown can be newer. Conflict checks are optimistic: another process
can still write in the small interval between the final check and replacement.

## Build and run

Requires Rust 1.95 (pinned in `rust-toolchain.toml`) and Node.js 24 or newer.
Windows builds also need the Visual Studio C++ build tools and the WebView2
runtime. The native egui source editor is available when a webview cannot be
created, and is the default Linux interface.

```powershell
npm ci
node tools/build-ui.cjs
cargo run
```

On Ubuntu 22.04 or newer, install native build dependencies first:

```bash
sudo apt-get install build-essential pkg-config libx11-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
npm ci
npm run build:ui
cargo run --locked
```

`npm run build:ui` also runs the UI build. For frontend development:

```powershell
node tools/build-ui.cjs --watch
```

Restart Markora after a UI rebuild. Debug builds load the generated on-disk
HTML on startup. Release builds embed it, so build the UI before `cargo build
--release`. Fonts are embedded in the executable and loaded on demand through
the local asset protocol. Node and a web server are not needed at runtime.

## Shortcuts (Windows web interface)

| Shortcut                  | Action                       |
| ------------------------- | ---------------------------- |
| Ctrl+N / Ctrl+T           | New tab                      |
| Ctrl+O / Ctrl+Shift+O     | Open file / folder           |
| Ctrl+S / Ctrl+Shift+S     | Save / Save as               |
| Ctrl+W                    | Close tab                    |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab          |
| Ctrl+1 / Ctrl+2 / Ctrl+3  | Live / Source / Split        |
| Ctrl+B / Ctrl+I / Ctrl+L  | Bold / italic / link         |
| Ctrl+F / F3 / Shift+F3    | Find / next / previous match |
| Ctrl+H                    | Find and replace             |
| Ctrl+Z / Ctrl+Shift+Z     | Undo / redo                  |
| Ctrl+Shift+P / Ctrl+K     | Command palette              |
| Ctrl+D                    | Focus mode                   |
| F11                       | Fullscreen reading / exit    |
| Ctrl+Shift+L              | Toggle light / dark          |
| Ctrl+Shift+T              | Next Markdown theme          |
| Ctrl+,                    | Settings                     |
| Ctrl+backslash            | Toggle files sidebar         |

## Code organization

See [ARCHITECTURE.md](ARCHITECTURE.md) for the module map and data-flow rules.

- `src/app.rs` handles the lifecycle; `src/app/` separates commands, IPC, files,
  the fallback interface, and host tests.
- `src/model.rs`, `markdown.rs`, `protocol.rs`, and `storage.rs` are independently
  testable library modules.
- `assets/ui/src/` contains small JavaScript modules for the bridge, editor,
  commands, layout, chrome, dialogs and find.
- `assets/ui/styles/` separates tokens, shell layout, editor and dialog styling.
- `assets/ui/shell.html` is the HTML source. `assets/ui/index.html` and
  `assets/ui/tailwind/app.built.css` are generated; edit their source files.

Each handwritten Rust and frontend JavaScript source module is kept below 300
lines; the UI verification command enforces that limit.

## Checks

```powershell
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
node tools/verify-ui.cjs
node --test tools/test-search.cjs
cargo build
node tools/smoke-native.cjs
node tools/smoke-safety.cjs
node tools/smoke-images.cjs
node tools/smoke-images.cjs --dom
node tools/smoke-find.cjs
```

The Windows native suite runs the actual Rust host and WebView2 interface. It checks menu
and keyboard dispatch, palette, source/split/live editing, formatting, undo,
find, sidebar resizing and persistence, full-width layouts, unsaved changes and
file open/save. It saves screenshots and diagnostics under `target/ui-smoke/`.
Tests use isolated settings and fixtures.
The safety suite also force-stops and restarts the host to verify draft recovery,
external-change dialogs, reload while editing a Live block, and discard on exit.
The find suite checks search options, captures, Unicode, replacement/undo/save,
worker cancellation and timeouts, stale results and all three Markdown modes.

Preferences and recent files are stored in a single `.markora/` directory next to
the `markora` executable, with an AppData fallback when that directory is
unavailable. `MARKORA_DATA_DIR` selects an alternate settings directory.
Markdown documents stay at their selected file paths.

## Publishing and license

GitHub Actions checks and packages Windows and Linux builds on pushes and pull
requests. A matching `v0.2.0` tag publishes both downloads and SHA-256 checksums to
GitHub Releases. See [PUBLISHING.md](PUBLISHING.md) for the clean first-upload
procedure, release commands and platform limitations.

Markora is MIT licensed. See [LICENSE](LICENSE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for bundled dependency notices.
