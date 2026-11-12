// Exercise the real host, lazy tree and Live edit gate using isolated files/settings.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const net = require("node:net");
const { spawn } = require("node:child_process");
const { connect, delay } = require("./cdp-client.cjs");

async function main() {
  const root = path.resolve(__dirname, "..");
  const artifacts = path.join(root, "target/ui-smoke", `sidebar-${Date.now()}`);
  const folder = path.join(artifacts, "workspace");
  const nested = path.join(folder, "Notes", "Deep");
  fs.mkdirSync(nested, { recursive: true });
  const note = path.join(nested, "note.md");
  const original = "# Nested note\n\nRead this paragraph.\n\n- [ ] Read-only task\n";
  fs.writeFileSync(note, original);
  fs.writeFileSync(path.join(folder, "root.md"), "# Root note\n");
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
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=* --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows`,
    },
  });
  let logs = "";
  app.stdout.on("data", (chunk) => {
    logs += chunk;
  });
  app.stderr.on("data", (chunk) => {
    logs += chunk;
  });
  let client;
  try {
    client = await connect(port);
    const { evaluate, wait, call } = client;
    const click = (selector) =>
      evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    const command = (id) => click(`[data-command="${id}"]`);
    const post = (action) =>
      evaluate(`window.ipc.postMessage(JSON.stringify(${JSON.stringify(action)}))`);
    const row = (name) => `#files [role="treeitem"][title$="${name}"]`;
    const key = async (key, modifiers = 0) => {
      await call("Input.dispatchKeyEvent", { type: "keyDown", key, modifiers });
      await call("Input.dispatchKeyEvent", { type: "keyUp", key, modifiers });
    };
    await wait(`document.querySelector('#connection')?.textContent==='Local'`, "handshake");
    await post({ type: "openPath", path: folder });
    await wait(`document.querySelectorAll('#files [role="treeitem"]').length===2`, "root tree");
    await evaluate(`document.querySelector(${JSON.stringify(row("Notes"))}).focus()`);
    await key("ArrowRight");
    await wait(`document.querySelector(${JSON.stringify(row("Deep"))})`, "lazy nested directory");
    await key("ArrowRight");
    assert.equal(await evaluate(`document.activeElement.title.endsWith('Deep')`), true);
    await key("ArrowRight");
    await wait(`document.querySelector(${JSON.stringify(row("note.md"))})`, "deep file");
    await click(row("note.md"));
    await wait(
      `document.querySelector('#live-document h1')?.textContent==='Nested note'`,
      "nested document",
    );
    assert.equal(
      await evaluate(`document.querySelector('#folder-path-label').textContent`),
      "workspace",
    );
    assert.equal(
      await evaluate(
        `document.querySelector(${JSON.stringify(row("note.md"))}).getAttribute('aria-level')`,
      ),
      "3",
    );
    await click(row("Notes"));
    assert.equal(
      await evaluate(`document.querySelector(${JSON.stringify(row("Deep"))})===null`),
      true,
    );
    await click(row("Notes"));
    assert.equal(
      await evaluate(`!!document.querySelector(${JSON.stringify(row("note.md"))})`),
      true,
    );
    await wait(
      `document.querySelector('#recents button')?.textContent.includes('note.md')`,
      "recent file",
    );
    await click("#toggle-recents");
    assert.equal(await evaluate(`document.querySelector('#recents').hidden`), true);
    await click("#toggle-recents");
    console.log(
      "PASS lazy tree, keyboard expansion, collapse/restore, root preservation and recent panel",
    );

    assert.equal(await evaluate(`document.querySelector('#live-access').textContent`), "Read-only");
    assert.equal(await evaluate(`document.querySelector('#append-block').hidden`), true);
    await click('#live-document [data-block="1"]');
    await key("Enter");
    assert.equal(await evaluate(`!!document.querySelector('.block-editor')`), false);
    assert.equal(
      await evaluate(`document.querySelector('#live-document input[type="checkbox"]').disabled`),
      true,
    );
    await command("bold");
    await key("b", 2);
    await command("replace");
    assert.equal(await evaluate(`document.querySelector('#replace-row').hidden`), true);
    await command("find");
    await evaluate(
      `(()=>{const t=document.querySelector('#find-input');t.value='paragraph';t.dispatchEvent(new Event('input'));})()`,
    );
    await wait(
      `document.querySelector('#find-count').textContent.includes('/ 1')`,
      "find while read-only",
    );
    await command("find-next");
    assert.equal(await evaluate(`!!document.querySelector('.block-editor')`), false);
    await command("find-close");
    await command("save");
    await delay(250);
    assert.equal(fs.readFileSync(note, "utf8"), original);
    console.log(
      "PASS read-only blocks, keyboard formatting, task lists, replacement and find navigation",
    );

    await key("e", 2);
    assert.equal(
      await evaluate(`document.querySelector('#toggle-edit').getAttribute('aria-pressed')`),
      "true",
    );
    await click('#live-document [data-block="1"]');
    await wait(`document.querySelector('.block-editor')`, "explicit edit mode");
    await evaluate(
      `(()=>{const t=document.querySelector('.block-editor');t.value=${JSON.stringify("Edited paragraph.\n")};t.dispatchEvent(new Event('input'));})()`,
    );
    await click("#toggle-edit");
    await wait(
      `!document.querySelector('.block-editor') && document.querySelector('#live-document').textContent.includes('Edited paragraph.')`,
      "finish edit",
    );
    await command("save");
    await wait(`document.querySelector('#save-state').textContent==='Saved'`, "saved edit");
    assert(fs.readFileSync(note, "utf8").includes("Edited paragraph."));
    assert.equal(
      await evaluate(`document.querySelector('#folder-path-label').textContent`),
      "workspace",
    );
    await wait(
      `document.querySelector(${JSON.stringify(row("note.md"))})`,
      "expanded tree after save",
    );
    await click("#toggle-edit");
    await click('#live-document input[type="checkbox"]');
    await command("save");
    await delay(250);
    assert(fs.readFileSync(note, "utf8").includes("- [x] Read-only task"));
    await click(row("root.md"));
    await wait(
      `document.querySelector('#live-document h1')?.textContent==='Root note'`,
      "switch document",
    );
    assert.equal(
      await evaluate(`document.querySelector('#toggle-edit').getAttribute('aria-pressed')`),
      "false",
    );
    await click("#toggle-edit");
    await command("view-source");
    await wait(`document.querySelector('#document').dataset.mode==='source'`, "source editor");
    await command("view-live");
    await wait(`document.querySelector('#document').dataset.mode==='live'`, "live mode reset");
    assert.equal(
      await evaluate(`document.querySelector('#toggle-edit').getAttribute('aria-pressed')`),
      "false",
    );
    await click('#recents button[title$="note.md"]');
    await wait(
      `document.querySelector('#live-document h1')?.textContent==='Nested note'`,
      "recent opens document",
    );
    const screenshot = await call("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(
      path.join(artifacts, "sidebar-readonly.png"),
      Buffer.from(screenshot.data, "base64"),
    );
    await command("new-tab");
    await wait(`document.querySelector('#live-document').textContent===''`, "empty document");
    assert.equal(await evaluate(`!!document.querySelector('.block-editor')`), false);
    await click("#toggle-edit");
    await wait(`document.querySelector('.block-editor')`, "empty note editing");
    assert.deepEqual(client.errors, []);
    console.log(
      "PASS explicit editing, done/flush/save, tasks, tab/view reset, recent open and empty note gate",
    );
    console.log(`Artifacts: ${artifacts}`);
  } catch (error) {
    if (client) {
      const screenshot = await client
        .call("Page.captureScreenshot", { format: "png" })
        .catch(() => null);
      if (screenshot)
        fs.writeFileSync(
          path.join(artifacts, "failure.png"),
          Buffer.from(screenshot.data, "base64"),
        );
      fs.writeFileSync(
        path.join(artifacts, "failure.txt"),
        await client.evaluate("document.body.innerText").catch(() => ""),
      );
    }
    throw error;
  } finally {
    fs.writeFileSync(path.join(artifacts, "native.log"), logs);
    client?.close();
    app.kill();
  }
}
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
