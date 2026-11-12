// Bundle the UI modules; fonts load on demand from the host's embedded assets.
// Source modules use named imports/exports; dependency order is explicit and checked.
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const root = path.resolve(__dirname, "..", "assets", "ui");
const order = [
  "dom",
  "icons",
  "bridge",
  "live-tasks",
  "live-editor",
  "editor-images",
  "editor",
  "reading",
  "layout",
  "command-list",
  "search-engine",
  "search-worker",
  "find",
  "dialogs",
  "conflicts",
  "images",
  "commands",
  "chrome",
  "main",
];
const exportsByModule = new Map();
const bodies = order.map((name) => {
  let source = fs.readFileSync(path.join(root, "src", `${name}.js`), "utf8");
  const exports = [...source.matchAll(/^export (?:const|function) ([\w$]+)/gm)].map((m) => m[1]);
  exportsByModule.set(name, exports);
  source = source
    .replace(
      /^import \{ ([^}]+) \} from ['"]\.\/([^'"]+)\.js['"];?\r?$/gm,
      (_, names, dependency) => {
        const available = exportsByModule.get(dependency);
        if (!available || names.split(",").some((n) => !available.includes(n.trim())))
          throw new Error(`Invalid import in ${name}: ${dependency}`);
        return `const { ${names} } = modules[${JSON.stringify(dependency)}];`;
      },
    )
    .replace(/^export /gm, "");
  return `modules[${JSON.stringify(name)}] = (() => {\n${source}\nreturn { ${exports.join(", ")} };\n})();`;
});

const js = `(() => {\n'use strict';\nconst modules = {};\n${bodies.join("\n")}\n})();`;
new vm.Script(js, { filename: "markora-ui.js" });
let css = fs.readFileSync(path.join(root, "tailwind", "app.built.css"), "utf8");
css = css.replace(/url\(["']?\.\.\/fonts\/([^"')]+)["']?\)/g, (_, name) => {
  return `url("/app/fonts/${name}")`;
});

const html = fs
  .readFileSync(path.join(root, "shell.html"), "utf8")
  .replace("<!-- STYLES -->", () => `<style>\n${css}\n</style>`)
  .replace(
    "<!-- SCRIPTS -->",
    () => `<script>\n${js.replace(/<\/script/gi, "<\\/script")}\n</script>`,
  );
fs.writeFileSync(path.join(root, "index.html"), html);
console.log(
  `Built ${order.length} UI modules with local font URLs (${Math.round(Buffer.byteLength(html) / 1024)} KiB).`,
);
