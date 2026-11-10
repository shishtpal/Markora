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
sudo apt-get install build-essential pkg-config libx11-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
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
--release`. Fonts are embedded in the bundle; Node and a web server are not
needed at runtime.

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
| Ctrl+Z / Ctrl+Shift+Z     | Undo / redo                  |
| Ctrl+Shift+P / Ctrl+K     | Command palette              |
| Ctrl+D                    | Focus mode                   |
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
cargo build
node tools/smoke-native.cjs
```

The Windows native suite runs the actual Rust host and WebView2 interface. It checks menu
and keyboard dispatch, palette, source/split/live editing, formatting, undo,
find, sidebar resizing and persistence, full-width layouts, unsaved changes and
file open/save. It saves screenshots and diagnostics under `target/ui-smoke/`.
Tests use isolated settings and fixtures.

Preferences and recent files are stored in `.markora/`, with an AppData fallback
when the local directory is unavailable. `MARKORA_DATA_DIR` selects an alternate
settings directory. Markdown documents stay at their selected file paths.

## Publishing and license

GitHub Actions checks and packages Windows and Linux builds on pushes and pull
requests. A matching `v0.2.0` tag publishes both downloads and SHA-256 checksums to
GitHub Releases. See [PUBLISHING.md](PUBLISHING.md) for the clean first-upload
procedure, release commands and platform limitations.

Markora is MIT licensed. See [LICENSE](LICENSE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for bundled dependency notices.
