# Publishing Markora

## First public upload

Use a fresh GitHub repository and the clean source snapshot:

```powershell
powershell -ExecutionPolicy Bypass -File tools/package-source.ps1
```

Extract `dist/markora-0.2.4-alpha-source.zip` into a new folder. Create an empty GitHub
repository, then publish that extracted folder with GitHub Desktop or Git.
The ZIP includes `.github/workflows/build.yml` and `.gitignore`; preserve hidden
files when extracting or copying. GitHub's web upload may omit hidden files, so
prefer a Git client for the initial upload. This project is an application;
`publish = false` in Cargo.toml only disables publishing it as a crates.io package.

The source export uses an explicit file list. It excludes `.git`, `.markora`,
local build outputs, dependencies, internal work notes and alternate pnpm files.
It does not stage files, create commits, or contact GitHub. Your working settings
remain unchanged.

Earlier local Git commits contain Microsoft Segoe UI font files. Deleting those
files in the current version does not remove them from previous commits. The
recommended fresh repository avoids distributing that history. The existing
repository also tracks `.markora/prefs.json`; `.gitignore` does not retroactively
untrack it. The clean export excludes all preferences and recent file paths.

## Automatic builds and releases

Every branch push and pull request runs checks and builds Windows x86_64 and
Linux x86_64 packages. Manual builds are available under **Actions → Build and
release → Run workflow** once the workflow is on the default branch. Download
the packages from the completed run's **Artifacts** section (retained 14 days).

To publish a release, keep Cargo.toml, the Cargo.lock root package, package.json
and both root version entries in package-lock.json in sync. Review and commit
the version changes yourself, push the commit, then push its matching `v` tag.
Review and commit all changes intended for the release, including new files and
generated UI bundles. Then run from the repository root on `main`:

```powershell
node tools/check-release.cjs
git diff --check
git push origin main
git tag -a v0.2.4-alpha -m "Markora v0.2.4-alpha"
git push origin v0.2.4-alpha
```

Only tag a commit containing the complete publishing setup. Both platform jobs
must succeed before the workflow publishes a GitHub Release. It uploads:

- `markora-0.2.4-alpha-windows-x86_64.zip` with `markora.exe`.
- `markora-0.2.4-alpha-linux-x86_64.tar.gz` with `markora`.
- A SHA-256 checksum file for each archive.

Archives include the README, project license, third-party notices and font
licenses. Tags containing a hyphen, such as `v0.3.0-beta.1`, become prereleases
when they match the package version. Re-running a successful tag workflow updates
its release assets. Branch builds never publish releases.

No personal access token or signing secret is required. GitHub supplies the
temporary repository token; only the final release job requests `contents: write`.
Repository/organization policy must allow GitHub Actions and that permission.

## Platform scope

Windows provides the full Live, Source and Split interface through WebView2.
Install the Microsoft Edge WebView2 Evergreen Runtime if it is missing. The ZIP
is a portable, unsigned application, without an installer or automatic updates.
The executable also requires the Microsoft Visual C++ v14 x64 Redistributable
(`VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`); runtime download links are in the README.

Linux currently provides the native egui source editor, tabs, file/folder dialogs,
save controls, outline and light/dark styling. The Windows web interface is not
ported to Linux. It needs an X11 or Wayland desktop and OpenGL/EGL. Builds use
Ubuntu 22.04/glibc 2.35; use a compatible or newer distribution. No WebKit/GTK
runtime is needed. File dialogs use the desktop's XDG portal.

The workflow runs Rust tests, Clippy and UI bundle checks on both platforms, plus
a headless native startup check on Linux using software OpenGL. Run the Windows
WebView2 smoke suite locally with `node tools/smoke-native.cjs` after `cargo build`;
it needs a desktop with a working graphics driver. A first successful GitHub run
is still required to verify the packages on GitHub-hosted machines.

## Local release packaging

```powershell
npm ci
npm run build:ui
cargo build --release --locked
node tools/collect-licenses.cjs
./tools/package-release.ps1
```

Use PowerShell 7 (`pwsh`) on Linux to run the packaging script. Build on the
target operating system; the script packages a host-native x86_64 binary.
