// Real WebView2 + Rust: workspace settings, typography, draft safety and persistence.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const net = require("node:net");
const { spawn } = require("node:child_process");
const { connect } = require("./cdp-client.cjs");
const root = path.resolve(__dirname, "..");

async function main() {
  const artifacts = path.join(root, "target/ui-smoke", `settings-${Date.now()}`);
  fs.mkdirSync(artifacts, { recursive: true });
  const note = path.join(artifacts, "typography.md");
  fs.writeFileSync(note, "# A little room to think\n\nA comfortable writing space.\n");
  const server = net.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  const app = spawn(path.join(root, "target/debug/markora.exe"), [], {
    cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, MARKORA_DATA_DIR: path.join(artifacts, "settings"),
      WEBVIEW2_USER_DATA_FOLDER: path.join(artifacts, "webview"),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=* --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows` },
  });
  let logs = "", client;
  app.stdout.on("data", (c) => { logs += c; });
  app.stderr.on("data", (c) => { logs += c; });
  try {
    client = await connect(port);
    const { evaluate, wait, call } = client;
    await evaluate(`window.settingsKeys=[];document.addEventListener('keydown',e=>window.settingsKeys.push([e.key,e.ctrlKey,e.shiftKey,e.defaultPrevented,e.target.id]),true)`);
    const click = (selector) => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    const cmd = (id) => click(`[data-command="${id}"]`);
    const change = (id, value) => evaluate(`(() => { const el=document.querySelector(${JSON.stringify(id)}); el.value=${JSON.stringify(value)}; el.dispatchEvent(new Event('input')); el.dispatchEvent(new Event('change')); })()`);
    const key = async (key, modifiers = 0) => {
      await call("Input.dispatchKeyEvent", { type: "keyDown", key, modifiers });
      await call("Input.dispatchKeyEvent", { type: "keyUp", key, modifiers });
    };
    const shot = async (name) => {
      await evaluate("document.fonts.ready");
      const result = await call("Page.captureScreenshot", { format: "png" });
      fs.writeFileSync(path.join(artifacts, `${name}.png`), Buffer.from(result.data, "base64"));
    };
    await wait(`document.querySelector('#connection')?.textContent==='Local'`, "handshake");
    await evaluate(`window.ipc.postMessage(${JSON.stringify(JSON.stringify({ type: "openPath", path: note }))})`);
    await wait(`document.title.includes('typography.md')`, "fixture document");
    await cmd("view-source");
    await wait(`document.querySelector('#document').dataset.mode==='source'`, "Source mode");
    await wait(`document.activeElement===document.querySelector('#source-editor')`, "Source focus restored");
    const draft = "# Unsaved draft\n\nKeep my text and selection.\n";
    await evaluate(`(() => { const el=document.querySelector('#source-editor'); el.value=${JSON.stringify(draft)}; el.dispatchEvent(new Event('input')); el.focus(); el.setSelectionRange(5,12); })()`);
    await key(",", 2);
    await wait(`!document.querySelector('#settings-page').hidden`, "Ctrl+, opens settings");
    await cmd("settings");
    assert.equal(await evaluate(`document.querySelectorAll('#settings-tab').length`), 1);
    assert.equal(await evaluate(`document.querySelectorAll('#tabs [aria-selected="true"]').length`), 1);
    assert.equal(await evaluate(`getComputedStyle(document.querySelector('#workspace')).display`), "none");
    await change("#font-family", "georgia");
    await change("#font-size", "22");
    await wait(`getComputedStyle(document.querySelector('#source-editor')).fontFamily.includes('Georgia')`, "font applied to Source");
    await wait(`getComputedStyle(document.querySelector('#font-preview')).fontSize==='22px'`, "font size applied");
    await key("w", 2);
    assert.equal(await evaluate(`document.querySelector('#settings-page').hidden`), true);
    assert.equal(await evaluate(`document.querySelector('#settings-tab')===null`), true);
    assert.equal(await evaluate(`document.querySelector('#source-editor').value`), draft);
    assert.deepEqual(await evaluate(`[document.querySelector('#source-editor').selectionStart,document.querySelector('#source-editor').selectionEnd]`), [5, 12]);
    console.log("PASS single Settings tab, shortcuts, Source font and unsaved draft/selection preservation");

    await cmd("settings");
    await evaluate(`[...document.querySelectorAll('#tabs [role="tab"]:not(#settings-tab)')].at(-1).click()`);
    await wait(`document.querySelector('#settings-page').hidden`, "return to document tab");
    assert.equal(await evaluate(`!!document.querySelector('#settings-tab')`), true);
    await key("Tab", 2);
    await wait(`!document.querySelector('#settings-page').hidden`, "Ctrl+Tab includes Settings");
    await key("Tab", 10);
    await wait(`document.querySelector('#settings-page').hidden`, "Ctrl+Shift+Tab returns to document");
    await cmd("settings");
    await click('.settings-nav a[href="#settings-typography"]');
    await change("#font-size", "99");
    await wait(`document.querySelector('#font-size').value==='32'`, "size upper bound");
    await change("#font-size", "");
    assert.equal(await evaluate(`document.querySelector('#font-size').value`), "32");
    await change("#font-size", "20");
    await wait(`getComputedStyle(document.querySelector('#font-preview')).fontSize==='20px'`, "font size saved");
    for (const theme of ["paper", "mono", "github"]) {
      await change("#md-theme", theme);
      await wait(`document.documentElement.dataset.mdTheme===${JSON.stringify(theme)}`, "theme changed");
      assert.equal(await evaluate(`getComputedStyle(document.querySelector('#live-document')).fontSize`), "20px");
      assert(await evaluate(`getComputedStyle(document.querySelector('#live-document')).fontFamily.includes('Georgia')`));
    }
    await change("#font-family", "ibm-plex-mono");
    await change("#color-mode", "light");
    await wait(`document.documentElement.dataset.color==='light'`, "light mode saved");
    await call("Emulation.setDeviceMetricsOverride", { width: 1200, height: 900, deviceScaleFactor: 1, mobile: false });
    await evaluate(`document.querySelector('#settings-page').scrollTop=0`);
    await shot("settings-light");
    await change("#color-mode", "dark");
    await wait(`document.documentElement.dataset.color==='dark'`, "dark mode saved");
    await shot("settings-dark");
    for (const width of [760, 600, 480]) {
      await call("Emulation.setDeviceMetricsOverride", { width, height: 800, deviceScaleFactor: 1, mobile: false });
      assert(await evaluate(`document.querySelector('#settings-page').scrollWidth<=document.querySelector('#settings-page').clientWidth`), `no overflow at ${width}px`);
    }
    await shot("settings-narrow");
    await change("#font-scale", "1.5");
    assert(await evaluate(`document.querySelector('#settings-page').scrollWidth<=document.querySelector('#settings-page').clientWidth`), "no overflow at maximum scale");
    await click("#reset-typography");
    await wait(`document.querySelector('#font-family').value==='theme' && document.querySelector('#font-size').value==='16'`, "reset typography");
    await change("#font-family", "georgia");
    await change("#font-size", "22");
    await key("Escape");
    await cmd("view-live");
    await wait(`document.querySelector('#document').dataset.mode==='live'`, "Live mode");
    assert(await evaluate(`getComputedStyle(document.querySelector('#live-document')).fontFamily.includes('Georgia')`));
    await cmd("toggle-edit");
    await click('#live-document [data-block="1"]');
    await wait(`document.querySelector('.block-editor')`, "Live block editor");
    assert.equal(await evaluate(`getComputedStyle(document.querySelector('.block-editor')).fontSize`), "22px");
    await cmd("settings");
    await cmd("view-split");
    await wait(`document.querySelector('#document').dataset.mode==='split' && document.querySelector('#settings-page').hidden`, "mode command exits Settings");
    assert.equal(await evaluate(`document.querySelector('#source-editor').value`), draft);
    const prefs = JSON.parse(fs.readFileSync(path.join(artifacts, "settings/prefs.json"), "utf8"));
    assert.equal(prefs.fontFamily, "georgia");
    assert.equal(prefs.fontSize, 22);
    await call("Page.reload");
    await wait(`document.querySelector('#connection')?.textContent==='Local'`, "reload handshake");
    await cmd("settings");
    await wait(`document.querySelector('#font-family').value==='georgia' && document.querySelector('#font-size').value==='22'`, "restored preferences");
    assert.deepEqual(client.errors, []);
    console.log("PASS tab navigation, size validation, all themes, responsive light/dark layouts, typography reset, Live/Split fonts and persisted preferences");
    console.log(`Artifacts: ${artifacts}`);
  } catch (error) {
    if (client) {
      console.error(await client.evaluate(`JSON.stringify({keys:window.settingsKeys,focus:document.activeElement?.outerHTML,settings:document.querySelector('#settings-page').hidden})`).catch(() => ""));
      const shot = await client.call("Page.captureScreenshot").catch(() => null);
      if (shot) fs.writeFileSync(path.join(artifacts, "failure.png"), Buffer.from(shot.data, "base64"));
      fs.writeFileSync(path.join(artifacts, "failure.txt"), await client.evaluate("document.body.innerText").catch(() => ""));
    }
    throw error;
  } finally {
    fs.writeFileSync(path.join(artifacts, "native.log"), logs);
    client?.close();
    app.kill();
  }
}
main().catch((error) => { console.error(error); process.exitCode = 1; });
