// Keyboard regressions against the production Rust host and WebView2.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const net = require("node:net");
const { spawn, spawnSync } = require("node:child_process");
const { connect } = require("./cdp-client.cjs");
const root = path.resolve(__dirname, "..");
async function main() {
  if (process.platform !== "win32") throw Error("Requires Windows and WebView2");
  const artifacts = path.join(root, "target/editing-smoke", String(Date.now()));
  fs.mkdirSync(artifacts, { recursive: true });
  const fixture = path.join(artifacts, "editing.md");
  fs.writeFileSync(fixture, "- first");
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
    const { call, evaluate, wait } = client;
    const click = (selector) =>
      evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    const command = (id) => click(`[data-command="${id}"]`);
    const key = async (key, code, modifiers = 0) => {
      await call("Input.dispatchKeyEvent", {
        type: "keyDown",
        key,
        code,
        modifiers,
        ...(key === "Enter" ? { text: "\r", windowsVirtualKeyCode: 13 } : {}),
      });
      await call("Input.dispatchKeyEvent", { type: "keyUp", key, code, modifiers: 0 });
    };
    const select = (selector, start, end = start, direction = "none") =>
      evaluate(`(() => {
      const t=document.querySelector(${JSON.stringify(selector)});t.focus();
      t.setSelectionRange(${start},${end},${JSON.stringify(direction)});
    })()`);
    const value = (selector) =>
      evaluate(`document.querySelector(${JSON.stringify(selector)}).value`);
    const is = (selector, text) =>
      wait(
        `document.querySelector(${JSON.stringify(selector)})?.value === ${JSON.stringify(text)}`,
        text,
      );
    const setSource = (text) =>
      evaluate(`(() => {const t=document.querySelector('#source-editor');
      t.value=${JSON.stringify(text)};t.dispatchEvent(new Event('input',{bubbles:true}));t.focus();t.setSelectionRange(t.value.length,t.value.length);})()`);
    const mode = async (name) => {
      await command("view-" + name);
      await wait(
        `document.querySelector('#document').dataset.mode===${JSON.stringify(name)}`,
        name,
      );
    };
    await wait(
      "document.querySelector('#connection')?.classList.contains('connected')",
      "connected",
    );
    await evaluate(
      `window.ipc.postMessage(${JSON.stringify(JSON.stringify({ type: "openPath", path: fixture }))})`,
    );
    await wait("document.title.includes('editing.md')", "fixture opened");
    await mode("source");
    for (const name of ["source", "split"]) {
      await mode(name);
      await setSource("9. [x] done");
      await key("Enter", "Enter");
      await is("#source-editor", "9. [x] done\n10. [ ] ");
      await key("Enter", "Enter");
      await is("#source-editor", "9. [x] done\n");
      await key("z", "KeyZ", 2);
      await is("#source-editor", "9. [x] done\n10. [ ] ");
      await key("z", "KeyZ", 10);
      await is("#source-editor", "9. [x] done\n");
      await setSource("- one\n- two\nlast");
      await select("#source-editor", 0, 12, "backward");
      await key("Tab", "Tab");
      await is("#source-editor", "    - one\n    - two\nlast");
      assert.equal(
        await evaluate("document.querySelector('#source-editor').selectionDirection"),
        "backward",
      );
      await key("Tab", "Tab", 8);
      await is("#source-editor", "- one\n- two\nlast");
      await setSource("```md\n- literal");
      await key("Enter", "Enter");
      await is("#source-editor", "```md\n- literal\n");
      console.log(
        `PASS ${name} list continuation/exit, grouped undo/redo, multiline indentation and fences`,
      );
    }
    const table = "| A | B |\n| --- | --- |\n| one | two |";
    await setSource(table);
    await select("#source-editor", table.indexOf("B"));
    await key("Tab", "Tab");
    assert.equal(
      await evaluate(
        "document.querySelector('#source-editor').value.slice(document.querySelector('#source-editor').selectionStart,document.querySelector('#source-editor').selectionEnd)",
      ),
      "one",
    );
    await key("Tab", "Tab", 8);
    assert.equal(
      await evaluate("document.querySelector('#source-editor').selectionStart"),
      table.indexOf("B"),
    );
    await key("z", "KeyZ", 2);
    await is("#source-editor", "```md\n- literal\n");
    await key("z", "KeyZ", 10);
    await is("#source-editor", table);
    await select("#source-editor", table.indexOf("two"));
    await key("Tab", "Tab");
    await is("#source-editor", table + "\n|   |   |");
    await call("Input.insertText", { text: "new" });
    const newTable = table + "\n| new  |   |";
    await is("#source-editor", newTable);
    await wait(
      "document.querySelectorAll('#preview-document tbody tr').length===2",
      "table preview",
    );
    console.log("PASS table navigation skips separators, appends rows and updates Split preview");
    await mode("live");
    assert.equal(await evaluate("document.querySelector('.block-editor')"), null);
    await click("#toggle-edit");
    await click("#live-document [data-block]");
    await wait("document.querySelector('.block-editor')", "Live table editor");
    await select(".block-editor", newTable.indexOf("two"));
    await key("Enter", "Enter");
    assert.equal(
      await evaluate(
        "(() => {const t=document.querySelector('.block-editor');return t.value.slice(t.selectionStart,t.selectionEnd);})()",
      ),
      "",
    );
    await call("Input.insertText", { text: "live" });
    await key("Escape", "Escape");
    await wait(
      "document.querySelector('#live-document table')?.textContent.includes('live')",
      "rendered Live table",
    );
    await mode("source");
    const saved = await value("#source-editor");
    await key("s", "KeyS", 2);
    await wait("document.querySelector('#save-state').textContent==='Saved'", "saved");
    assert.equal(fs.readFileSync(fixture, "utf8"), saved);
    console.log("PASS Live table edits persist through rendering, mode switch and Save");
    await setSource("- live item");
    await mode("live");
    await click("#toggle-edit");
    await click("#live-document [data-block]");
    await wait("document.querySelector('.block-editor')", "Live list editor");
    await select(".block-editor", 11);
    await key("Enter", "Enter");
    assert((await value(".block-editor")).startsWith("- live item\n- "));
    await key("Escape", "Escape");
    await click("#append-block");
    await wait("document.querySelector('.block-editor')", "new Live paragraph");
    await call("Input.insertText", { text: "- new item" });
    await key("Enter", "Enter");
    await is(".block-editor", "- new item\n- ");
    await key("Tab", "Tab");
    await is(".block-editor", "- new item\n    - ");
    await key("Tab", "Tab", 8);
    await is(".block-editor", "- new item\n- ");
    const screenshot = await call("Page.captureScreenshot");
    fs.writeFileSync(
      path.join(artifacts, "live-editing.png"),
      Buffer.from(screenshot.data, "base64"),
    );
    assert.deepEqual(client.errors, []);
    console.log("PASS existing and appended Live blocks share list and indentation behavior");
    console.log(`Artifacts: ${artifacts}`);
  } catch (error) {
    if (client) {
      try {
        fs.writeFileSync(
          path.join(artifacts, "failure.json"),
          JSON.stringify(
            await client.evaluate(
              "({body:document.body.innerText,source:document.querySelector('#source-editor').value,live:document.querySelector('.block-editor')?.value})",
            ),
            null,
            2,
          ),
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
