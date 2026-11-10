// Include upstream notices from the exact locked Rust sources used by this host.
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const root = path.resolve(__dirname, '..');
const upstream = require('../licenses/rust-upstream.json');
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8', windowsHide: true })
  .match(/^host: (.+)$/m)[1].trim();
const metadata = JSON.parse(execFileSync('cargo', [
  'metadata', '--locked', '--format-version', '1', '--filter-platform', process.argv[2] || host,
], { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, windowsHide: true }));
const resolved = new Set(metadata.resolve.nodes.map((node) => node.id));
const packages = metadata.packages.filter((pkg) => pkg.source && resolved.has(pkg.id))
  .sort((a, b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`));
function licenseFiles(directory, nested = false) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory() && !nested && /^(licenses?|legal)$/i.test(entry.name))
      return licenseFiles(filename, true);
    return entry.isFile() && /^(licen[cs]e|copying|notice|copyright|unlicense)/i.test(entry.name)
      ? [filename] : [];
  });
}
const report = ['Markora third-party Rust dependency notices',
  'Generated from Cargo.lock; includes build and test dependencies.', ''];
for (const pkg of packages) {
  const directory = path.dirname(pkg.manifest_path);
  const files = new Set(licenseFiles(directory));
  if (pkg.license_file) files.add(path.resolve(directory, pkg.license_file));
  report.push('='.repeat(72), `${pkg.name} ${pkg.version}`, `License: ${pkg.license || 'See upstream license file'}`,
    `Source: ${pkg.repository || `https://crates.io/crates/${pkg.name}/${pkg.version}`}`, '');
  for (const file of files) {
    report.push(`--- ${path.relative(directory, file)} ---`, fs.readFileSync(file, 'utf8'), '');
  }
  if (!files.size) {
    const vendored = upstream[`${pkg.name}@${pkg.version}`];
    if (!vendored?.length) throw new Error(`Missing upstream license text: ${pkg.name}@${pkg.version}`);
    for (const file of vendored) report.push(`--- ${file.path} ---`, `From: ${file.source}`,
      fs.readFileSync(path.join(root, file.path), 'utf8'), '');
  }
}
fs.mkdirSync(path.join(root, 'target'), { recursive: true });
fs.writeFileSync(path.join(root, 'target/THIRD_PARTY_LICENSES.txt'), report.join('\n'));
console.log(`Collected notices for ${packages.length} locked Rust dependencies.`);
