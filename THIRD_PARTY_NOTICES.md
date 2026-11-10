# Third-party notices

Markora source code is distributed under the MIT license in `LICENSE`.
Dependencies retain their own licenses. Release archives include
`THIRD_PARTY_LICENSES.txt`, generated from the locked Rust dependency sources.
For crates that omit license files from their package archive, `licenses/rust/`
contains upstream texts; `licenses/rust-upstream.json` records their exact source
revisions. Refresh these entries when upgrading those dependencies.
Tailwind CSS is MIT licensed; its generated CSS retains the upstream notice.

The web interface bundles unmodified IBM Plex Mono, Noto Sans Devanagari and
Space Grotesk fonts under the SIL Open Font License 1.1. Their copyright and
license texts are in `assets/ui/fonts/LICENSE-*.txt` in source distributions and
`licenses/` in binary distributions.

The native interface uses egui's default Hack, Noto Emoji, Ubuntu Light and
emoji-icon-font fonts. Their upstream license texts are included in `licenses/`.
Markora does not bundle Microsoft Segoe UI fonts. The Windows web interface may
use the system-installed Segoe UI font through CSS.

Markora is an independent project inspired by single-document Markdown editing.
It is not affiliated with Typora. The GitHub Markdown theme is independently
implemented; its color reference is linked in the README.
