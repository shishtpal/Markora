const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const version = cargo.match(/^version = "([^"]+)"/m)?.[1];
assert(version, 'Cargo.toml must declare a version');
assert.equal(require('../package.json').version, version, 'Rust and UI versions must match');
if (process.env.GITHUB_REF_TYPE === 'tag') {
  assert.equal(process.env.GITHUB_REF_NAME, `v${version}`, 'Release tag must match Cargo.toml');
}
console.log(`Release version: ${version}`);
