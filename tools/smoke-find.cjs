// Real Rust + WebView2 search, replacement, undo, cancellation and save regressions.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const net = require("node:net");
const { spawn, spawnSync } = require("node:child_process");
const { connect, delay } = require("./cdp-client.cjs");
const root = path.resolve(__dirname, "..");

async function main() {
  if (process.platform !== "win32") throw new Error("Requires Windows and WebView2");
  const artifacts = path.join(root, "target/find-smoke", String(Date.now()));
  fs.mkdirSync(artifacts, { recursive: true });
  const fixture = path.join(artifacts, "find.md");
  const original = "cat Cat scatter cat_ cat";
  fs.writeFileSync(fixture, original);
  const server = net.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  const app = spawn(path.join(root, "target/debug/markora.exe"), [], {
    cwd: root,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      ...process.env,
      MARKORA_DATA_DIR: path.join(artifacts, "settings"),
      WEBVIEW2_USER_DATA_FOLDER: path.join(artifacts, "webview"),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=* --disable-background-timer-throttling --disable-renderer-backgrounding`,
    },
  });
  let client,
    logs = "";
  app.stdout.on("data", (chunk) => {
    logs += chunk;
  });
  app.stderr.on("data", (chunk) => {
    logs += chunk;
  });
  try {
    client = await connect(port);
    const { evaluate, wait, call } = client;
    const click = (selector) =>
      evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    const input = (selector, value) =>
      evaluate(
        `(() => { const t = document.querySelector(${JSON.stringify(selector)}); t.value = ${JSON.stringify(value)}; t.dispatchEvent(new Event('input', { bubbles: true })); })()`,
      );
    const menu = async (name, command) => {
      await evaluate(
        `[...document.querySelectorAll('.menu-trigger')].find(t => t.textContent === ${JSON.stringify(name)}).click()`,
      );
      await click(`.dropdown:not([hidden]) [data-command="${command}"]`);
    };
    const options = (values) =>
      evaluate(
        `(() => { for (const [id, value] of Object.entries(${JSON.stringify(values)})) { const b = document.querySelector('#find-' + id); if ((b.getAttribute('aria-pressed') === 'true') !== value) b.click(); } })()`,
      );
    const count = (n) =>
      wait(
        `document.querySelector('#find-count').textContent.endsWith(${JSON.stringify(" / " + n)})`,
        `${n} matches`,
      );
    const textIs = (text) =>
      wait(
        `document.querySelector('#source-editor').value === ${JSON.stringify(text)}`,
        "source text",
      );
    const messageIs = (text) =>
      wait(
        `document.querySelector('#find-message').textContent === ${JSON.stringify(text)}`,
        "replacement status",
      );
    const key = async (key, code, modifiers = 0) => {
      await call("Input.dispatchKeyEvent", { type: "keyDown", key, code, modifiers });
      await call("Input.dispatchKeyEvent", { type: "keyUp", key, code, modifiers: 0 });
    };
    const mode = async (value) => {
      await menu("View", "view-" + value);
      await wait(
        `document.querySelector('#document').dataset.mode === ${JSON.stringify(value)}`,
        value + " mode",
      );
    };
    await wait(
      `document.querySelector('#connection')?.classList.contains('connected')`,
      "connected",
    );
    await evaluate(
      `window.ipc.postMessage(${JSON.stringify(JSON.stringify({ type: "openPath", path: fixture }))})`,
    );
    await wait(`document.title.includes('find.md')`, "open fixture");
    await mode("source");
    await textIs(original);
    await key("h", "KeyH", 2);
    await wait(
      `!document.querySelector('#findbar').hidden && !document.querySelector('#replace-row').hidden`,
      "Ctrl+H opens replacement",
    );
    assert.equal(await evaluate("document.activeElement.id"), "find-input");
    await input("#find-input", "cat");
    await count(5);
    await options({ case: true });
    await count(4);
    await options({ word: true });
    await count(2);
    await key("F3", "F3", 8);
    await wait(
      `document.querySelector('#source-editor').selectionStart === ${original.lastIndexOf("cat")}`,
      "previous wraps to final whole-word match",
    );
    await key("F3", "F3");
    await wait(
      `document.querySelector('#source-editor').selectionStart === 0`,
      "next wraps to first match",
    );
    await input("#replace-input", "fox");
    await click("#replace-one");
    await textIs("fox Cat scatter cat_ cat");
    await messageIs("Replaced 1 match.");
    await menu("Edit", "undo");
    await textIs(original);
    await input("#replace-input", "dog");
    await count(2);
    await click("#replace-all");
    await textIs("dog Cat scatter cat_ dog");
    await messageIs("Replaced 2 matches.");
    await menu("Edit", "undo");
    await textIs(original);
    await menu("Edit", "redo");
    await textIs("dog Cat scatter cat_ dog");
    await menu("File", "save");
    for (
      let attempt = 0;
      attempt < 100 && fs.readFileSync(fixture, "utf8") !== "dog Cat scatter cat_ dog";
      attempt++
    )
      await delay(50);
    await wait(`!document.querySelector('.tab.active .dirty-dot')`, "saved replacement");
    assert.equal(fs.readFileSync(fixture, "utf8"), "dog Cat scatter cat_ dog");
    console.log(
      "PASS Ctrl+H, case/whole-word toggles, wrapped navigation, replace current/all, single-step undo/redo and real save",
    );

    const captures = "x=1\nx=22\n😀 x=333";
    await input("#source-editor", captures);
    await options({ case: false, word: false, regex: true });
    await input("#find-input", "x=(\\d+)");
    await input("#replace-input", "y=$1");
    await count(3);
    await click('[data-command="find-next"]');
    await click('[data-command="find-next"]');
    await wait(
      `document.querySelector('#source-editor').value.slice(document.querySelector('#source-editor').selectionStart, document.querySelector('#source-editor').selectionEnd) === 'x=22'`,
      "variable-length regex selection",
    );
    await click("#replace-one");
    await textIs("x=1\ny=22\n😀 x=333");
    await messageIs("Replaced 1 match.");
    await click("#replace-all");
    await textIs("y=1\ny=22\n😀 y=333");
    await menu("Edit", "undo");
    await textIs("x=1\ny=22\n😀 x=333");
    await menu("Edit", "undo");
    await textIs(captures);
    await input("#find-input", "(?<key>x)=(?<value>\\d+)");
    await input("#replace-input", "$<value>:$<key>");
    await count(3);
    const screenshot = await call("Page.captureScreenshot");
    fs.writeFileSync(
      path.join(artifacts, "find-replace.png"),
      Buffer.from(screenshot.data, "base64"),
    );
    await evaluate("document.querySelector('#replace-input').focus()");
    await key("Enter", "Enter", 2);
    await textIs("1:x\n22:x\n😀 333:x");
    console.log(
      "PASS variable-length regex matches, numbered/named captures and Ctrl+Enter replacement",
    );

    await input("#source-editor", "😀 नमस्ते नमस्तेपन café caféine नमस्ते");
    await options({ regex: false, word: true });
    await input("#find-input", "नमस्ते");
    await count(2);
    await input("#replace-input", "");
    await click("#replace-all");
    await textIs("😀  नमस्तेपन café caféine ");
    await input("#source-editor", "İx 😀 X");
    await options({ word: false });
    await input("#find-input", "x");
    await count(2);
    await click('[data-command="find-next"]');
    await wait(
      `document.querySelector('#source-editor').selectionStart === 1`,
      "Unicode case folding preserves source offsets",
    );
    await input("#source-editor", "😀");
    await options({ regex: true });
    await input("#find-input", "(?:)");
    await input("#replace-input", "|");
    await count(2);
    await click("#replace-all");
    await textIs("|😀|");
    console.log(
      "PASS Unicode whole words, deletion, accurate UTF-16 offsets and zero-length regex replacement",
    );

    await input("#find-input", "[");
    await wait(
      `document.querySelector('#find-input').getAttribute('aria-invalid') === 'true'`,
      "invalid regex feedback",
    );
    assert.equal(await evaluate("document.querySelector('#replace-all').disabled"), true);
    await input("#source-editor", "a".repeat(64) + "!");
    await input("#find-input", "(a+)+$");
    await wait(
      `document.querySelector('#find-count').textContent === 'Searching…'`,
      "slow regex starts",
    );
    assert.equal(await evaluate("1 + 1"), 2, "UI thread remains responsive");
    await input("#find-input", "a");
    await count(64);
    await input("#find-input", "(a+)+$");
    await wait(
      `document.querySelector('#find-message').textContent.includes('too long')`,
      "regex timeout",
      5000,
    );
    await input("#find-input", "a");
    await count(64);
    console.log(
      "PASS invalid regex feedback, worker cancellation, timeout and recovery without freezing the UI",
    );

    await options({ regex: false });
    await input("#source-editor", "old old");
    await input("#find-input", "old");
    await input("#replace-input", "new");
    await count(2);
    await evaluate(
      `(() => { window.originalSearchPost = Worker.prototype.postMessage; Worker.prototype.postMessage = function(data, ...rest) { if (data.operation === 'replaceAll') setTimeout(() => window.originalSearchPost.call(this, data, ...rest), 200); else return window.originalSearchPost.call(this, data, ...rest); }; })()`,
    );
    await click("#replace-all");
    await menu("File", "new-tab");
    await textIs("");
    await delay(500);
    await textIs("");
    await evaluate("Worker.prototype.postMessage = window.originalSearchPost");
    await evaluate(
      `[...document.querySelectorAll('.tab-title')].find(t => t.textContent === 'find.md').click()`,
    );
    await textIs("old old");
    console.log("PASS a replacement result cannot overwrite a newly activated document");

    await input("#source-editor", "hello hello");
    await mode("live");
    await click("#toggle-edit");
    await input("#find-input", "hello");
    await input("#replace-input", "world");
    await count(2);
    await click('[data-command="find-next"]');
    await wait(`document.querySelector('.block-editor')`, "live match is editable");
    await click("#replace-all");
    await wait(
      `document.querySelector('#live-document').textContent.includes('world world')`,
      "live replacement renders",
    );
    await mode("split");
    await textIs("world world");
    await wait(
      `document.querySelector('#preview-document').textContent.includes('world world')`,
      "split preview updates",
    );
    await menu("Edit", "undo");
    await textIs("hello hello");
    await wait(
      `document.querySelector('#preview-document').textContent.includes('hello hello')`,
      "split preview undoes replacement",
    );
    await click("#find-input");
    await key("Escape", "Escape");
    await wait("document.querySelector('#findbar').hidden", "Escape closes find");
    await menu("Edit", "replace");
    await wait("!document.querySelector('#replace-row').hidden", "Edit menu opens replacement");
    assert.deepEqual(client.errors, []);
    console.log(
      "PASS Live/Source/Split consistency, preview updates, undo and keyboard/menu controls",
    );

    // A PDF hides the search bar without using its Close action; pending work must cancel.
    const pdf = path.join(artifacts, "read-only.pdf");
    const objects = [
      "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
      "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
      "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    ];
    let pdfText = "%PDF-1.4\n";
    const offsets = [];
    for (const object of objects) {
      offsets.push(pdfText.length);
      pdfText += object;
    }
    const xref = pdfText.length;
    pdfText +=
      "xref\n0 4\n0000000000 65535 f \n" +
      offsets.map((at) => `${String(at).padStart(10, "0")} 00000 n \n`).join("");
    pdfText += `trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
    fs.writeFileSync(pdf, pdfText);
    await options({ regex: false, word: false });
    await evaluate(
      `Worker.prototype.postMessage = function(data, ...rest) { setTimeout(() => window.originalSearchPost.call(this, data, ...rest), 250); }`,
    );
    await input("#find-input", "hello");
    await wait(
      `document.querySelector('#find-count').textContent === 'Searching…'`,
      "pending search before PDF",
    );
    await evaluate(
      `window.ipc.postMessage(${JSON.stringify(JSON.stringify({ type: "openPath", path: pdf }))})`,
    );
    await wait(
      `document.querySelector('#app').dataset.docKind === 'pdf' && document.querySelector('#findbar').hidden`,
      "PDF hides search",
    );
    await menu("Edit", "replace");
    assert.equal(await evaluate("document.querySelector('#findbar').hidden"), true);
    await evaluate("Worker.prototype.postMessage = window.originalSearchPost");
    await evaluate(
      `[...document.querySelectorAll('.tab-title')].find(t => t.textContent === 'find.md').click()`,
    );
    await wait(
      `document.querySelector('#app').dataset.docKind === 'markdown'`,
      "return to Markdown",
    );
    await menu("Edit", "replace");
    await count(2);
    assert.deepEqual(client.errors, []);
    console.log(
      "PASS PDF read-only controls and interrupted search resumes after returning to Markdown",
    );
    console.log(`Artifacts: ${artifacts}`);
  } catch (error) {
    if (client) {
      try {
        fs.writeFileSync(
          path.join(artifacts, "failure-state.json"),
          JSON.stringify(
            await client.evaluate(
              `({body:document.body.innerText,source:document.querySelector('#source-editor').value})`,
            ),
            null,
            2,
          ),
        );
        const screenshot = await client.call("Page.captureScreenshot");
        fs.writeFileSync(
          path.join(artifacts, "failure.png"),
          Buffer.from(screenshot.data, "base64"),
        );
      } catch {}
    }
    throw error;
  } finally {
    client?.close();
    fs.writeFileSync(path.join(artifacts, "runtime.log"), logs);
    if (app.exitCode === null)
      spawnSync("taskkill", ["/pid", String(app.pid), "/T", "/F"], { windowsHide: true });
  }
}
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
