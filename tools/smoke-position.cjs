// Exercise the production Rust host and WebView2, including delayed Live rendering.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const net = require("node:net");
const { spawn } = require("node:child_process");
const { connect } = require("./cdp-client.cjs");
const root = path.resolve(__dirname, "..");

async function main() {
  if (process.platform !== "win32") throw Error("Requires Windows and WebView2");
  const artifacts = path.join(root, "target/position-smoke", String(Date.now()));
  fs.mkdirSync(artifacts, { recursive: true });
  const markdown = Array.from(
    { length: 1000 },
    (_, i) => `Paragraph ${i} 😀 ${"text ".repeat(20)}\n\n`,
  ).join("");
  const files = ["position-a.md", "position-b.md"].map((name) => path.join(artifacts, name));
  for (const file of files) fs.writeFileSync(file, markdown);
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
    const { evaluate, wait } = client;
    const send = (message) =>
      evaluate(`window.ipc.postMessage(${JSON.stringify(JSON.stringify(message))})`);
    const click = (selector) =>
      evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    const command = (command) => send({ type: "command", command });
    const frames = () =>
      evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))");
    const mode = async (value) => {
      await command("view-" + value);
      await wait(
        `document.querySelector('#document').dataset.mode === ${JSON.stringify(value)}`,
        value,
      );
      await frames();
    };
    const tab = async (index) => {
      await evaluate(
        `[...document.querySelectorAll('.tab-title')].find(b => b.textContent === ${JSON.stringify(path.basename(files[index]))}).click()`,
      );
      await wait(
        `document.title.includes(${JSON.stringify(path.basename(files[index]))})`,
        "tab active",
      );
      await frames();
    };
    const sourceState = () =>
      evaluate(
        `(() => {const t=document.querySelector('#source-editor'); return [t.selectionStart,t.selectionEnd,t.selectionDirection,t.scrollTop,t.scrollLeft];})()`,
      );
    const setSource = async (start, end, top) => {
      await evaluate(
        `(() => {const t=document.querySelector('#source-editor'); t.focus(); t.setSelectionRange(${start},${end},'backward');t.scrollTop=${top};t.scrollLeft=25;})()`,
      );
      return sourceState();
    };
    await wait(
      "document.querySelector('#connection')?.classList.contains('connected')",
      "connected",
    );
    await send({ type: "openPath", path: files[0] });
    await wait("document.title.includes('position-a.md')", "A open");
    await mode("source");
    const aSource = await setSource(3020, 3035, 2100);
    assert(aSource[3] > 0);
    await send({ type: "openPath", path: files[1] });
    await wait("document.title.includes('position-b.md')", "B open");
    await mode("source");
    assert.deepEqual((await sourceState()).slice(0, 2), [0, 0]);
    assert.equal((await sourceState())[3], 0);
    const bSource = await setSource(120, 133, 700);
    await tab(0);
    assert.deepEqual(await sourceState(), aSource);
    await tab(1);
    assert.deepEqual(await sourceState(), bSource);
    await command("toggle-theme");
    await frames();
    assert.deepEqual(
      await sourceState(),
      bSource,
      "unrelated state echo preserves cursor and scroll",
    );
    console.log(
      "PASS Source selections, backward direction, independent scroll and same-content tabs",
    );

    await tab(0);
    await mode("split");
    await evaluate("document.querySelector('#preview-pane').scrollTop=1800");
    const aSplit = await sourceState();
    const previewTop = await evaluate("document.querySelector('#preview-pane').scrollTop");
    assert(previewTop > 0);
    await tab(1);
    await mode("split");
    await evaluate("document.querySelector('#preview-pane').scrollTop=400");
    await tab(0);
    assert.deepEqual(await sourceState(), aSplit);
    assert.equal(await evaluate("document.querySelector('#preview-pane').scrollTop"), previewTop);
    console.log("PASS Split restores both independent panes");

    await mode("live");
    await wait("document.querySelector('[data-block=\"850\"]')", "deep block rendered");
    await evaluate(
      "document.querySelector('[data-block=\"850\"]').scrollIntoView({block:'start'})",
    );
    await frames();
    const liveOffset = () =>
      evaluate(
        "document.querySelector('[data-block=\"850\"]').getBoundingClientRect().top-document.querySelector('#live-pane').getBoundingClientRect().top",
      );
    const anchor = await liveOffset();
    await tab(1);
    await tab(0);
    const afterAnchor = await liveOffset();
    assert(
      Math.abs(afterAnchor - anchor) <= 2,
      `deep Live anchor moved from ${anchor} to ${afterAnchor}`,
    );
    assert.equal(
      await evaluate("document.querySelector('#toggle-edit').getAttribute('aria-pressed')"),
      "false",
    );
    await click("#toggle-edit");
    await click('[data-block="850"]');
    await evaluate(
      "(() => {const t=document.querySelector('.block-editor');t.setSelectionRange(13,21,'backward');})()",
    );
    // Keep an unsent-to-renderer block mutation in the active textarea while switching.
    await evaluate(
      "(() => {const t=document.querySelector('.block-editor');t.value=t.value.replace('text','changed');t.dispatchEvent(new Event('input'));t.setSelectionRange(13,21,'backward');})()",
    );
    const liveSelection = () =>
      evaluate(
        "(() => {const t=document.querySelector('.block-editor');return [t?.selectionStart,t?.selectionEnd,t?.selectionDirection,t?.value];})()",
      );
    let selection = await liveSelection();
    await tab(1);
    await tab(0);
    await wait(
      "document.querySelector('[data-block=\"850\"] .block-editor')",
      "Live block recreated",
    );
    assert.deepEqual(await liveSelection(), selection);
    assert.equal(
      await evaluate("document.querySelector('#toggle-edit').getAttribute('aria-pressed')"),
      "true",
    );
    assert.equal(await evaluate("document.activeElement.className"), "block-editor");
    console.log(
      "PASS Deep Live anchor, edit mode, UTF-16 block selection and edited draft restore",
    );

    await evaluate("document.querySelector('.block-editor').setSelectionRange(17,23,'forward')");
    selection = await liveSelection();
    await command("toggle-reading");
    await wait("document.body.classList.contains('reading-mode')", "reading");
    await evaluate("document.querySelector('#preview-pane').scrollTop=1200");
    const readingTop = await evaluate("document.querySelector('#preview-pane').scrollTop");
    await tab(1);
    await evaluate("document.querySelector('#preview-pane').scrollTop=300");
    await tab(0);
    assert.equal(await evaluate("document.querySelector('#preview-pane').scrollTop"), readingTop);
    await command("toggle-reading");
    await wait("!document.body.classList.contains('reading-mode')", "exit reading");
    await frames();
    assert.deepEqual(await liveSelection(), selection);
    console.log("PASS Reading preview is per tab and preserves the editing selection");

    await tab(1);
    // Rapid activation must never apply a queued restore to the wrong tab.
    await evaluate(
      `(() => { const buttons=[...document.querySelectorAll('.tab-title')]; for(let i=0;i<6;i++){buttons.find(b=>b.textContent==='position-a.md').click();buttons.find(b=>b.textContent==='position-b.md').click();} })()`,
    );
    await frames();
    await tab(1);
    assert.deepEqual(await sourceState(), bSource);
    await command("close-tab");
    await wait("!document.title.includes('position-b.md')", "B closed");
    await send({ type: "openPath", path: files[1] });
    await wait("document.title.includes('position-b.md')", "B reopened");
    await mode("source");
    assert.deepEqual((await sourceState()).slice(0, 2), [0, 0]);
    assert.equal((await sourceState())[3], 0, "closed tab position is released");
    await tab(0);
    await wait("document.querySelector('.block-editor')", "A cursor survives closing B");
    assert.deepEqual(await liveSelection(), selection);
    await click("#append-block");
    await frames();
    const emptyOffset = () =>
      evaluate(
        "document.querySelector('.block-editor').getBoundingClientRect().top-document.querySelector('#live-pane').getBoundingClientRect().top",
      );
    const emptyTop = await emptyOffset();
    await tab(1);
    await tab(0);
    await wait(
      "document.querySelector('.block-editor')?.value === ''",
      "empty new paragraph restored",
    );
    const restoredEmptyTop = await emptyOffset();
    assert(
      Math.abs(restoredEmptyTop - emptyTop) <= 2,
      `empty paragraph offset moved from ${emptyTop} to ${restoredEmptyTop}`,
    );
    assert.deepEqual(client.errors, []);
    assert.equal(await evaluate("document.querySelector('#error-banner').hidden"), true);
    console.log(
      "PASS Rapid activation, closed-tab cleanup, empty paragraph and no JavaScript errors",
    );
  } catch (error) {
    error.message += `\nArtifacts: ${artifacts}\n${logs}`;
    throw error;
  } finally {
    client?.close();
    app.kill();
  }
}
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
