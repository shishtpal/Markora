// Runs inside smoke-native's isolated Rust/WebView2 process, with real Windows keys.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { delay } = require("./cdp-client.cjs");

function nativeWindow(processId, action = "Inspect") {
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-File",
    path.join(__dirname, "reading-window.ps1"), "-ProcessId", String(processId), "-Action", action],
    { windowsHide: true, encoding: "utf8", timeout: 30000 });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr || result.stdout);
  return JSON.parse(result.stdout.trim());
}

module.exports = async function checkReading(client, menu, processId, artifacts) {
  const { evaluate, wait, call } = client;
  const native = (action) => nativeWindow(processId, action);
  async function key(key, extra = {}) {
    const virtual = { Escape: 27, Enter: 13, F11: 122, ",": 188 }[key] || key.toUpperCase().charCodeAt(0);
    const code = key.length === 1 ? key === "," ? "Comma" : `Key${key.toUpperCase()}` : key;
    const event = { key, code, windowsVirtualKeyCode: virtual, nativeVirtualKeyCode: virtual };
    await call("Input.dispatchKeyEvent", { type: "keyDown", ...event, ...extra });
    await call("Input.dispatchKeyEvent", { type: "keyUp", ...event, modifiers: 0 });
  }
  const reading = `document.body.classList.contains('reading-mode')`;
  const source = `document.querySelector('#source-editor')`;
  const baseline = await evaluate(`${source}.value`);
  const original = native();
  for (const mode of ["source", "split", "live"]) {
    await menu("View", `view-${mode}`);
    await wait(`document.querySelector('#document').dataset.mode==='${mode}'`, `${mode} before reading`);
    if (mode === "live") {
      await evaluate(`document.querySelector('.live-block[data-block="1"]').click()`);
    }
    const textarea = mode === "live" ? `document.querySelector('.block-editor')` : source;
    await evaluate(`(()=>{const t=${textarea};t.focus();t.setSelectionRange(5,12);window.readingEditor=t;})()`);
    const prefsFile = path.join(artifacts, "settings", "prefs.json");
    const prefs = fs.readFileSync(prefsFile, "utf8");
    native("F11");
    await wait(reading, `${mode} native F11 entry`);
    await wait(`document.querySelector('#preview-document strong')?.textContent==='bold'`, "rendered reading text");
    assert.equal(await evaluate(`getComputedStyle(document.querySelector('#preview-pane')).display`), "block");
    for (const id of ["menubar", "tabbar", "viewbar", "statusbar", "source-pane", "live-pane", "files-panel", "outline-panel"]) {
      assert.equal(await evaluate(`getComputedStyle(document.querySelector('#${id}')).display`), "none", `${id} hidden`);
    }
    await delay(150);
    const fullscreen = native();
    assert.deepEqual(fullscreen.rect, fullscreen.monitor, "native window fills its monitor");
    native("F11Repeat");
    await evaluate(`document.querySelector('[data-command="bold"]').click()`);
    await key("z", { modifiers: 2 });
    await key("h", { modifiers: 2 });
    await key("F11", { autoRepeat: true });
    assert.equal(await evaluate(reading), true, "repeat cannot exit reading");
    if (mode === "source") {
      const screenshot = await call("Page.captureScreenshot", { format: "png" });
      fs.writeFileSync(path.join(artifacts, "reading.png"), Buffer.from(screenshot.data, "base64"));
    }
    assert.equal(await evaluate(`document.querySelector('#replace-row').hidden`), true);
    await key("f", { modifiers: 2 });
    await evaluate(`(()=>{const t=document.querySelector('#find-input');t.value='paragraph';t.dispatchEvent(new Event('input'));})()`);
    await wait(`document.querySelector('#find-count').textContent.includes('/ 1')`, "reading find results");
    await key("Enter");
    await wait(`!!document.querySelector('.reading-match')`, "find navigates rendered preview");
    await key("Escape");
    assert.equal(await evaluate(reading), true, "first Escape closes find");
    await key("Escape");
    await wait(`!(${reading})`, `${mode} Escape exit`);
    await wait(`document.activeElement===window.readingEditor`, "restored editor focus");
    assert.deepEqual(await evaluate(`[window.readingEditor.selectionStart,window.readingEditor.selectionEnd]`), [5, 12]);
    assert.equal(await evaluate(`${source}.value`), baseline);
    assert.equal(fs.readFileSync(prefsFile, "utf8"), prefs, "reading leaves preferences untouched");
    await delay(100);
    const restored = native();
    assert.deepEqual(restored.rect, original.rect, "native window bounds restored");
  }
  native("F11Parent");
  await wait(reading, "native parent F11 entry");
  native("F11Parent");
  await wait(`!(${reading})`, "native parent F11 exit");
  // Restore a maximized window too, and keep focus-mode preferences independent.
  native("Maximize");
  await delay(150);
  const maximized = native();
  await menu("View", "toggle-focus");
  await menu("View", "toggle-reading");
  await wait(reading, "menu reading entry");
  await key(",", { modifiers: 2 });
  await wait(`document.querySelector('#settings-dialog').open`, "reading settings dialog");
  await key("Escape");
  await wait(`!document.querySelector('#settings-dialog').open`, "Escape closes reading settings");
  assert.equal(await evaluate(reading), true, "dialog Escape preserves reading");
  native("F11");
  await wait(`!(${reading})`, "native F11 exit");
  assert.equal(await evaluate(`document.body.classList.contains('focus-mode')`), true);
  await delay(150);
  assert.equal(native().placement.showCmd, maximized.placement.showCmd, "maximized state restored");
  native("Restore");
  await menu("View", "toggle-focus");
  await menu("View", "view-split");
  console.log("PASS F11 native fullscreen, Live/Source/Split reading, read-only commands, find, Escape and editor/window restoration");
};

module.exports.pdf = async function checkPdfReading(client, processId) {
  const { evaluate, wait } = client;
  const reading = `document.body.classList.contains('reading-mode')`;
  await evaluate(`document.querySelector('#pdf-frame').focus()`);
  nativeWindow(processId, "F11");
  await wait(reading, "PDF-focused native F11 entry");
  assert.equal(await evaluate(`getComputedStyle(document.querySelector('#preview-pane')).display`), "none");
  const before = await evaluate(`document.querySelector('#pdf-frame').src`);
  const screenshot = await client.call("Page.captureScreenshot", { format: "png" });
  await evaluate(`document.querySelector('#pdf-frame').focus()`);
  nativeWindow(processId, "Escape");
  await wait(`!(${reading})`, "PDF-focused native Escape exit");
  assert.equal(await evaluate(`document.querySelector('#pdf-frame').src`), before);
  console.log("PASS PDF-focused native F11/Escape and stable PDF viewer");
  return screenshot;
};
