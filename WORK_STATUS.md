# Typora-inspired egui MVP

## Current refactor — 2026-11-09

- Replaced the monolithic app and inline frontend with small Rust and JavaScript modules. See ARCHITECTURE.md for the current module map; older notes below describe superseded implementations.
- Native IPC now wakes the egui event loop. A readiness handshake gates snapshots, Serde handles the protocol, and every command carries the latest document draft.
- One CommonMark parser supplies preview HTML, source-ranged Live blocks and the outline. Live block edits now update the actual Markdown; offsets use UTF-16 for JavaScript.
- Menus, shortcuts and palette share command IDs and dispatch. Unsaved changes are handled in visible HTML dialogs, with every affected document considered.
- Both sidebars and the Split divider resize. Sidebar widths persist, and explicit grid placement preserves full document width when panels hide.
- UI sources live in assets/ui/src, styles and shell.html. Build with node tools/build-ui.cjs; index.html is generated, with embedded fonts.
- Production regression tests replace the old mirrored bridge tests. Native WebView2 tests cover commands, editing, resize, full-width layout, unsaved Cancel/Discard and actual file save.
- No staging, unstaging or commits were performed by the assistant.

## Objective
Create an original Rust UI MVP inspired by Typora (not a 1:1 replica) in the current workspace, modeled after `D:\Codings\lang-Rust\egui-json-yaml-explorer`.

## User constraints / decisions
- Scope selected: MVP Markdown editor (file open/save, folder sidebar, Markdown edit pane, live preview, basic themes, search). No WYSIWYG parity with Typora.
- Target selected: current workspace (`D:\tmp\hello`) because harness file tools are restricted to it.
- Must not guess Typora internals; REA inspection of `Typora.exe` failed due size/budget and it is likely an Electron/native wrapper. Installed resources contain ASARs/extracted JS if user later wants targeted static inspection.
- No proprietary assets/code copying.

## Status (project lives in markora/)
- Markora = thin egui host (src/app.rs) + full WebView2 chrome (assets/ui/index.html).
- Multi-tab docs, prefs + recents (.markora/ -> %APPDATA%\Markora), bridge v2, egui fallback, 6 md themes, light/dark, command palette, find bar, focus mode, split view, drag-drop, unsaved dialog, status bar.
- Detailed plan: markora/WORK_STATUS.md (scaffolding done -> see completed below).

## Completed (this session)
- Read markora/WORK_STATUS.md + src (app.rs, state.rs, theme.rs, webview_host.rs, main.rs) + assets/ui/index.html + DESIGN.md. Confirmed app.rs is 1469 lines (rewrite deferred) and HTML already has full v2 chrome (929 lines).
- Removed dead code in app.rs: sync_prefs_to_doc() no-op loop, markdownChanged no-op view_mode line. Added #[allow(dead_code)] on Pending (webview/folder pending actions), Palette (full DESIGN.md), Recents::remove.
- Rust: bridge openFile now routes folder paths to open_folder (JS folder clicks previously misrouted to file-open).
- JS: folder click unified to post({type:"openFile"}); runCommand open-folder-path shim is now a no-op comment (Rust routes dirs).
- state.rs test: prefs_roundtrip uses struct initializer (fixes clippy::field_reassign_with_default).

## Checks (verified this session, markora/)
- `cargo build` — passes.
- `cargo clippy --all-targets` — passes (zero warnings, was 4).
- `cargo test` — passes (3 tests: prefs_roundtrip, recents_roundtrip_and_dedup, document_dirty_and_title).

## Next steps
- Manual UI smoke-test (`cargo run` on Windows with WebView2): tabs, recents, themes, split, folder open, unsaved dialog, keyboard shortcuts.
- Optional: wire Pending::New/OpenFile/OpenFolder to actual webview commands so the variants are constructed (currently guarded by #[allow(dead_code)]), and remove the allow once so.
- Update README.md with verified check commands + WebView2 requirement if desired.# Markora — continuation notes

## Reported issues (this session)
- Open file not working.
- Change theme (light/dark) not working.
- Font ugly; UI must follow DESIGN.md; Tailwind v4 allowed.

## Root causes found
1. **Theme double-toggle** — `assets/ui/index.html` `runCommand("toggle-theme")` sent `prefsChanged{colorMode}` AND `cmd("toggle-theme")`; Rust flipped once per message, cancelling out (no visible change). Also `applyTheme`'s `if(fontScale)` guard ignored scale on most calls and never synced `state.colorMode`, so subsequent toggles computed from stale state.
2. **Open file silent** — `post({type:"openFile"})` routing was correct in Rust (`is_dir → open_folder`), but JS gave zero feedback (no toast/title), used emoji glyphs, and `load_index_html` hot-reload + IPC timing hid errors. Folder/file clicks and recents all used the same untested path.
3. **Font/render** — `Style::dark()/light()` removed in egui 0.36 broke `cargo build` (`theme.rs`), `accent_soft`/`selection_fill` helpers were dead, no bundled fonts (relied on a single `segoeui.ttf` load), emoji glyph icons (`📁📄◐⌘⚙↻✕`), dummy `example.com` insert, gutter joined with `\n` without `white-space:pre`, and theme button showed no state.

## Fixes applied this session
- **Rust `src/theme.rs`**: clone `ctx.style_of(theme)` (egui 0.36 API), wire `selection_fill`/`accent_soft` (remove `#[allow(dead)]` warnings), DESIGN type scale (15/15/12/11), `load_fonts` prefers bundled Space Grotesk / IBM Plex Mono / Noto Sans Devanagari under `assets/ui/fonts/` with `segoeui.ttf` fallback.
- **WebView JS**: `toggle-theme` → single `prefsChanged`; `applyTheme` always sets `--font-size`, syncs `state.colorMode/mdTheme`; new `syncChrome()` renders Lucide inline SVGs, updates tab/document title, theme toggle `aria-pressed` + tooltip, and settings seg icons; file/folder/recents buttons show `Opening…` toasts + `title=path`; recents icon `history`; tab close uses `icon('x')`; `isdir` helper; link inserts via prompt.
- **DESIGN.md UI**: Tailwind v4 source `assets/ui/tailwind/app.css` (tokens, `@custom-variant dark` for `data-color`, `@theme inline` colors, `@font-face` ×6, all layout/editor/etc. CSS), built via `@tailwindcss/cli` 4.3.5, injected into `index.html` by `tools/inject-css.cjs`. No CDN. Paired tokens exactly as DESIGN.md table. `body` font `Space Grotesk → Segoe UI → Noto Sans Devanagari`.
- **Tests**: `tests/bridge_regression.rs` (8 tests) pins open routing + theme flip; `tools/verify-ui.cjs` asserts 20 UI invariants (PASS).

## Status
- `cargo build` ✓, `cargo clippy --all-targets` ✓ (0 warnings), `cargo test` ✓ (3 unit + 8 integration), `node tools/verify-ui.cjs` ✓ 20/20.
- Manual smoke-test still needed: `cargo run` on Windows with WebView2 — tabs, recents, folder open, unsaved dialog, shortcuts, focus mode, split drag.

## Next steps
- Manual UI smoke-test (see above).
- Optional: remove egui-fallback warnings if any; wire `Pending::New/OpenFile/OpenFolder` fully if desired.
- Commit the set (fonts are binary but small; `segoeui.ttf`+`segoeuib.ttf` are system fonts — consider dropping them and relying on woff2 + system fallback to reduce repo size).

## Constraints respected
- No proprietary Typora assets/code. Fonts bundled with licenses (`LICENSE-*.txt`). No external CDN at runtime.



## Font panic fix (2026-11-09 session)
- Root cause: src/theme.rs fed bundled *.woff2 (wOF2 0x774F4632) into egui FontData::from_owned; egui 0.36 skrifa accepts only raw TTF/OTF sfnt and panics at epaint/fonts.rs:990. segoeui.ttf/segoeuib.ttf are ## Menu functions not working (2026-11-09 follow-up)
- Two missed races: (1) the page `post()` can still run before `window.ipc`/`chrome.webview` exist (they arrive very late for a child-window WebView2) and `catch(e){}` hid the no-op; (2) `sync_webview` pushed `setState` on frame 1 before the HTML finished loading, so the first state/markdown/theme `evaluate_script` was dropped and every menu effect was a frame behind.
- Fixes: (1) `webview_host.rs` injects `BRIDGE_INIT_SCRIPT` at document-start via `with_initialization_script` - it queues in `window.__bridgeSend` and flushes when the handles appear, plus exposes `window.cefQuery`/`cefQueryCancel` pointing at the same handler queue. (2) `app.rs` gates `sync_webview` on wry page-load Finished (`page_loaded` flag), and in `ui()` `drain_bridge()` now runs before `sync_webview()` so a menu command's state change reaches the page in the same frame. (3) `index.html post()` prefers `__bridgeSend`, then `cefQuery`, then `ipc`/`chrome.webview`; boot logs `[bridge] page boot`.
- Verified live: `cargo run` prints `[bridge->rust] {"type":"ping"}` then `[bridge] page load finished: about:blank`; build/clippy/test/verify-ui 23/23.

