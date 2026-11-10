// Fast bundle checks. Actual UI behavior is tested by smoke-native.cjs.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const root = path.resolve(__dirname, "..");
const html = fs.readFileSync(path.join(root, "assets/ui/index.html"), "utf8");
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
new vm.Script(script);
const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((m) => m[1]);
assert.equal(ids.length, new Set(ids).size, "HTML IDs must be unique");
assert(html.includes("data:font/woff2;base64,"), "Fonts must be embedded for with_html navigation");
assert(!html.includes("../fonts/"), "No unresolved font paths");
assert(
  !html.includes("<!-- STYLES -->") && !html.includes("<!-- SCRIPTS -->"),
  "Bundle placeholders replaced",
);
for (const directory of ['assets/ui/src', 'src', 'src/app']) {
  for (const file of fs.readdirSync(path.join(root, directory), { withFileTypes:true })) {
    if (!file.isFile() || !/\.(js|rs)$/.test(file.name)) continue;
    const lines = fs.readFileSync(path.join(root, directory, file.name), 'utf8').split('\n').length;
    assert(lines <= 300, `${directory}/${file.name} has ${lines} lines; split it by responsibility`);
  }
}
console.log("PASS: bundle syntax, unique IDs, offline fonts and source modules under 300 lines.");
