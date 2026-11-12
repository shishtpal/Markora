// Actual WebView2 regression: external edits, killed host, restored drafts and discard.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const net = require("node:net");
const { spawn, spawnSync } = require("node:child_process");
const { connect, delay } = require("./cdp-client.cjs");
const root = path.resolve(__dirname, "..");

async function main() {
  if (process.platform !== "win32") throw new Error("Requires Windows and WebView2");
  const artifacts = path.join(root, "target/safety-smoke", String(Date.now()));
  const settings = path.join(artifacts, "settings");
  fs.mkdirSync(artifacts, { recursive: true });
  const note = path.join(artifacts, "notes.md");
  fs.writeFileSync(note, "original");
  let app, client, logs = "", run = 0;
  async function start() {
    const server = net.createServer();
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const port = server.address().port;
    await new Promise((resolve) => server.close(resolve));
    app = spawn(path.join(root, "target/debug/markora.exe"), [], {
      cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, MARKORA_DATA_DIR: settings,
        WEBVIEW2_USER_DATA_FOLDER: path.join(artifacts, `webview-${run++}`),
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=* --disable-background-timer-throttling --disable-renderer-backgrounding`,
      },
    });
    app.stdout.on("data", (chunk) => { logs += chunk; });
    app.stderr.on("data", (chunk) => { logs += chunk; });
    client = await connect(port);
    await client.wait(`document.querySelector('#connection')?.classList.contains('connected')`, "host connected", 10000);
  }
  async function stop() {
    client?.close();
    client = null;
    if (app && app.exitCode === null) {
      spawnSync("taskkill", ["/pid", String(app.pid), "/T", "/F"], { windowsHide: true });
      await delay(200);
    }
    app = null;
  }
  const send = (message) => client.evaluate(`window.ipc.postMessage(${JSON.stringify(JSON.stringify(message))})`);
  const cmd = (command) => send({ type: "command", command });
  const click = (selector) => client.evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
  const edit = (text) => client.evaluate(`(() => { const t = document.querySelector('#source-editor'); t.value = ${JSON.stringify(text)}; t.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  const conflict = () => client.wait(`document.querySelector('#conflict-dialog').open`, "external-change dialog", 8000);
  const closed = () => client.wait(`!document.querySelector('#conflict-dialog').open`, "conflict resolved");
  const sourceIs = (text) => client.wait(`document.querySelector('#source-editor').value === ${JSON.stringify(text)}`, "source text");
  const tabs = () => client.evaluate(`[...document.querySelectorAll('#tabs .tab-title')].map(t => t.textContent)`);
  async function checkpoint(text) {
    for (let count = 0; count < 100; count++) {
      try {
        const session = JSON.parse(fs.readFileSync(path.join(settings, "session.json"), "utf8"));
        if (session.tabs.some((t) => t.markdown === text)) return;
      } catch {}
      await delay(100);
    }
    throw new Error("Recovery checkpoint did not reach disk");
  }
  try {
    await start();
    await send({ type: "openPath", path: note });
    await client.wait(`document.title.includes('notes.md')`, "open fixture");
    await cmd("view-source");
    await sourceIs("original");
    await edit("my draft");
    fs.writeFileSync(note, "external");
    await conflict();
    const dialogShot = await client.call("Page.captureScreenshot");
    fs.writeFileSync(path.join(artifacts, "conflict.png"), Buffer.from(dialogShot.data, "base64"));
    await click('[data-conflict="cancel"]');
    await closed();
    await sourceIs("my draft");
    await cmd("save");
    await conflict();
    assert.equal(fs.readFileSync(note, "utf8"), "external");
    await click('[data-conflict="keep"]');
    await closed();
    assert.equal(fs.readFileSync(note, "utf8"), "my draft");
    console.log("PASS poll detects external edit; Later preserves draft; Save requires review; Keep Mine saves");

    // Reload must also reset a live block textarea, rather than leaving its old draft active.
    await cmd("view-live");
    await client.wait(`document.querySelector('#live-document [data-block]')`, "live block");
    await click("#live-document [data-block]");
    await client.wait(`document.querySelector('.block-editor')`, "active live block");
    fs.writeFileSync(note, "# Reloaded externally");
    await conflict();
    await click('[data-conflict="reload"]');
    await closed();
    await client.wait(`document.querySelector('#live-document').textContent.includes('Reloaded externally') && !document.querySelector('.block-editor')`, "reload replaces active live block");
    await cmd("view-source");
    await edit("saved file crash draft");
    await checkpoint("saved file crash draft");
    await cmd("new-tab");
    await cmd("view-split");
    await edit("# Untitled नमस्ते 😀 recovery");
    await checkpoint("# Untitled नमस्ते 😀 recovery");
    const order = await tabs();
    assert.deepEqual(client.errors, []);
    await stop(); // Force-kill the host after recovery has actually reached disk.
    fs.writeFileSync(note, "edited while Markora was closed");

    await start();
    await conflict();
    await click('[data-conflict="cancel"]');
    await closed();
    await sourceIs("# Untitled नमस्ते 😀 recovery");
    assert.deepEqual(await tabs(), order);
    assert.equal(await client.evaluate(`document.querySelector('#document').dataset.mode`), "split");
    await client.evaluate(`[...document.querySelectorAll('#tabs .tab-title')].find(t => t.textContent === 'notes.md').click()`);
    await sourceIs("saved file crash draft");
    assert.equal(fs.readFileSync(note, "utf8"), "edited while Markora was closed");
    console.log("PASS forced shutdown restores ordered tabs, active tab, view mode, Unicode untitled and saved-file drafts; offline edits are detected");

    await cmd("exit");
    for (let index = 0; index < 2; index++) {
      await client.wait(`document.querySelector('#unsaved-dialog').open`, "unsaved exit dialog");
      await click('[data-unsaved="discard"]');
      if (index === 0) await delay(100);
    }
    for (let count = 0; count < 100 && app.exitCode === null; count++) await delay(50);
    assert.equal(app.exitCode, 0, "graceful exit completes");
    await stop();
    await start();
    await client.wait(`document.querySelectorAll('.dirty-dot').length === 0`, "discarded drafts stay discarded");
    assert(!(await tabs()).some((title) => title.includes("Untitled नमस्ते")));
    await client.evaluate(`[...document.querySelectorAll('#tabs .tab-title')].find(t => t.textContent === 'notes.md').click()`);
    await sourceIs("edited while Markora was closed");
    assert.deepEqual(client.errors, []);
    console.log("PASS graceful exit preserves saved tabs without resurrecting discarded drafts");
    console.log(`Artifacts: ${artifacts}`);
  } catch (error) {
    if (client) {
      try {
        const screenshot = await client.call("Page.captureScreenshot");
        fs.writeFileSync(path.join(artifacts, "failure.png"), Buffer.from(screenshot.data, "base64"));
      } catch {}
    }
    throw error;
  } finally {
    fs.writeFileSync(path.join(artifacts, "runtime.log"), logs);
    await stop();
  }
}
main().catch((error) => { console.error(error); process.exitCode = 1; });
