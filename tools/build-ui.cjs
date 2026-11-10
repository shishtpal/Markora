// One build entry point; works with plain Node even without a global npm shim.
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const root = path.resolve(__dirname, '..');
function build() {
  const result = spawnSync(process.execPath, [
    path.join(root, 'node_modules/@tailwindcss/cli/dist/index.mjs'),
    '-i', 'assets/ui/tailwind/app.css', '-o', 'assets/ui/tailwind/app.built.css',
  ], { cwd: root, stdio: 'inherit', windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error('Tailwind build failed. Install the UI dependencies first.');
  delete require.cache[require.resolve('./bundle-ui.cjs')];
  require('./bundle-ui.cjs');
}
build();
if (process.argv.includes('--watch')) {
  let timer;
  function changed(_event, filename) {
    if (filename && !/\.(js|css|html)$/.test(filename.toString())) return;
    if (filename?.toString() === 'app.built.css') return;
    clearTimeout(timer);
    timer = setTimeout(() => { try { build(); } catch (error) { console.error(error.message); } }, 150);
  }
  for (const directory of ['src', 'styles', 'tailwind']) fs.watch(path.join(root, 'assets/ui', directory), changed);
  fs.watch(path.join(root, 'assets/ui/shell.html'), changed);
  console.log('Watching UI sources. Restart Markora after a rebuild to load the new bundle.');
}
