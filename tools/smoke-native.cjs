// End-to-end tests against the real Rust host + WebView2. No mock bridge.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const net = require("node:net");
const { spawn } = require("node:child_process");
const { connect, delay } = require("./cdp-client.cjs");
const root = path.resolve(__dirname, "..");

async function main() {
  if (process.platform !== "win32")
    throw new Error("Native UI smoke tests require Windows + WebView2");
  const server = net.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  const artifacts = path.join(root, "target", "ui-smoke", String(Date.now()));
  fs.mkdirSync(artifacts, { recursive: true });
  const fixture = path.join(artifacts, "fixture.md");
  fs.writeFileSync(fixture, "# Fixture\n\nOriginal.\n");
  const assetsDir = path.join(artifacts, "Todos.assets");
  fs.mkdirSync(assetsDir, { recursive: true });
  const wav = Buffer.alloc(44 + 8000);
  wav.write("RIFF", 0);
  wav.writeUInt32LE(36 + 8000, 4);
  wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(8000, 24);
  wav.writeUInt32LE(8000, 28);
  wav.writeUInt16LE(1, 32);
  wav.writeUInt16LE(8, 34);
  wav.write("data", 36);
  wav.writeUInt32LE(8000, 40);
  fs.writeFileSync(path.join(assetsDir, "Krishna Madam 2021-09-16 09-50-50.wav"), wav);
  const startupStart = Date.now();
  const app = spawn(path.join(root, "target/debug/markora.exe"), [], {
    cwd: root,
    windowsHide: true,
    env: {
      ...process.env,
      MARKORA_DATA_DIR: path.join(artifacts, "settings"),
      WEBVIEW2_USER_DATA_FOLDER: path.join(artifacts, "webview"),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=* --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows`,
    },
    stdio: ["ignore", "pipe", "pipe"],
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
    const { evaluate: evaluate, wait, call } = client;
    const click = (selector) =>
      evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    const menu = async (name, command) => {
      await evaluate(
        `[...document.querySelectorAll('.menu-trigger')].find(b=>b.textContent===${JSON.stringify(name)}).click()`,
      );
      await click(`.dropdown:not([hidden]) [data-command="${command}"]`);
    };
    const input = (text) =>
      evaluate(
        `(()=>{const t=document.querySelector('#source-editor'); t.value=${JSON.stringify(text)}; t.dispatchEvent(new Event('input',{bubbles:true}));})()`,
      );
    await wait(`document.querySelector('#connection')?.textContent==='Local'`, "native handshake");
    await wait(`document.querySelectorAll('.live-block').length>3`, "initial rendered document");
    console.log(`PASS native handshake and initial render (${Date.now() - startupStart} ms, fresh WebView2 profile)`);

    await menu("File", "new-tab");
    await wait(`document.querySelectorAll('.tab').length===2`, "File > New tab");
    await menu("View", "view-source");
    await wait(`document.querySelector('#document').dataset.mode==='source'`, "View > Source");
    const markdown = "# नमस्ते 😀\n\nA **bold** paragraph.\n\n## Second\n\n- One\n- Two\n";
    await input(markdown);
    // Immediate mode change carries the current draft, with no debounce race.
    await menu("View", "view-split");
    await wait(
      `document.querySelector('#preview-document strong')?.textContent==='bold'`,
      "split preview after edit",
    );
    assert.equal(await evaluate(`document.querySelector('#source-editor').value`), markdown);
    await menu("View", "toggle-theme");
    await wait(`document.documentElement.dataset.color==='light'`, "menu theme toggle");
    await call("Input.dispatchKeyEvent", {
      type: "keyDown",
      key: "L",
      code: "KeyL",
      modifiers: 10,
    });
    await call("Input.dispatchKeyEvent", { type: "keyUp", key: "L", code: "KeyL", modifiers: 10 });
    await wait(`document.documentElement.dataset.color==='dark'`, "keyboard theme toggle");
    console.log("PASS File/View menus, draft ordering, split preview, theme menu and shortcut");
    await require("./check-reading.cjs")(client, menu, app.pid, artifacts);

    await click('[data-command="command-palette"]');
    await evaluate(
      `(()=>{const t=document.querySelector('#palette-input');t.value='live editor';t.dispatchEvent(new Event('input'));})()`,
    );
    await click("#palette-list button");
    await wait(`document.querySelector('#document').dataset.mode==='live'`, "palette Live editor");
    await click('.live-block[data-block="1"]');
    const liveText = "A live edit with **persisted** Markdown.\n";
    await evaluate(
      `(()=>{const t=document.querySelector('.block-editor');t.value=${JSON.stringify(liveText)};t.dispatchEvent(new Event('input'));})()`,
    );
    await call("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape" });
    await call("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape" });
    await wait(
      `document.querySelector('#live-document strong')?.textContent==='persisted'`,
      "Live edit rendered",
    );
    await menu("View", "view-source");
    await wait(
      `document.querySelector('#document').dataset.mode==='source' && document.querySelector('#source-editor').value.includes('live edit with **persisted**')`,
      "Live edit reaches source",
    );
    console.log("PASS command palette and Live → Markdown → Source roundtrip with Unicode");

    await evaluate(
      `(()=>{const t=document.querySelector('#source-editor');t.focus();t.setSelectionRange(0,0);})()`,
    );
    await menu("Format", "heading-2");
    await wait(
      `document.querySelector('#source-editor').value.startsWith('## नमस्ते')`,
      "format source heading",
    );
    await menu("Edit", "undo");
    await wait(
      `document.querySelector('#source-editor').value.startsWith('# नमस्ते')`,
      "undo formatting",
    );
    await menu("Edit", "find");
    assert.equal(await evaluate(`document.activeElement.id`), "find-input");
    await evaluate(
      `(()=>{const t=document.querySelector('#find-input');t.value='persisted';t.dispatchEvent(new Event('input'));})()`,
    );
    await click('[data-command="find-next"]');
    await wait(`document.querySelector('#find-count').textContent==='1 / 1'`, "find next");
    await click('[data-command="find-close"]');
    console.log("PASS formatting selection, undo and find");

    for (const side of ["files", "outline"]) {
      const before = await evaluate(
        `document.querySelector('#${side}-panel').getBoundingClientRect().width`,
      );
      const rect = await evaluate(
        `(()=>{const r=document.querySelector('#${side}-resizer').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`,
      );
      await call("Input.dispatchMouseEvent", {
        type: "mousePressed",
        x: rect.x,
        y: rect.y,
        button: "left",
        clickCount: 1,
      });
      await call("Input.dispatchMouseEvent", {
        type: "mouseMoved",
        x: rect.x + (side === "files" ? 70 : -60),
        y: rect.y,
        button: "left",
        buttons: 1,
      });
      await call("Input.dispatchMouseEvent", {
        type: "mouseReleased",
        x: rect.x + (side === "files" ? 70 : -60),
        y: rect.y,
        button: "left",
        clickCount: 1,
      });
      await wait(
        `document.querySelector('#${side}-panel').getBoundingClientRect().width>${before + 30}`,
        `${side} pointer resize`,
      );
    }
    await wait(
      `document.querySelector('#files-resizer').getAttribute('aria-valuenow')==='310'`,
      "files width persisted",
    );
    let prefs;
    for (let attempt = 0; attempt < 100; attempt++) {
      prefs = JSON.parse(fs.readFileSync(path.join(artifacts, "settings/prefs.json"), "utf8"));
      if (prefs.filesWidth === 310 && prefs.outlineWidth === 280) break;
      await delay(50);
    }
    assert.equal(prefs.filesWidth, 310);
    assert.equal(prefs.outlineWidth, 280);
    console.log("PASS both sidebar pointer resizers and saved widths");

    await menu("View", "toggle-focus");
    await wait(
      `document.querySelector('#files-panel').hidden && document.querySelector('#outline-panel').hidden`,
      "focus mode",
    );
    await menu("View", "view-live");
    await wait(`document.querySelector('#document').dataset.mode==='live'`);
    await call('Emulation.setDeviceMetricsOverride', { width:1600, height:900, deviceScaleFactor:1, mobile:false });
    await delay(100);
    const widths = await evaluate(
      `({editor:document.querySelector('#document').clientWidth,live:document.querySelector('#live-document').getBoundingClientRect().width,window:innerWidth})`,
    );
    assert(widths.editor > 900);
    assert.equal(widths.live, widths.editor);
    await menu("View", "view-split");
    await wait(`document.querySelector('#document').dataset.mode==='split'`);
    assert(
      await evaluate(
        `document.querySelector('#source-pane').clientWidth + document.querySelector('#preview-pane').clientWidth >= document.querySelector('#document').clientWidth-6`,
      ),
    );
    console.log("PASS full-width Live and Split workspaces");
    await call('Emulation.clearDeviceMetricsOverride');

    await menu("File", "close-tab");
    await wait(
      `document.querySelector('#unsaved-dialog').open`,
      "unsaved dialog visible above editor",
    );
    await click('[data-unsaved="cancel"]');
    await wait(`!document.querySelector('#unsaved-dialog').open`, "cancel close");
    assert.equal(await evaluate(`document.querySelectorAll('.tab').length`), 2);
    await menu("File", "close-tab");
    await wait(`document.querySelector('#unsaved-dialog').open`);
    await click('[data-unsaved="discard"]');
    await wait(`document.querySelectorAll('.tab').length===1`, "discard closes tab");
    console.log("PASS unsaved changes Cancel and Discard");

    // Use the production transport to open a known local fixture without automating a system dialog.
    await evaluate(
      `window.ipc.postMessage(JSON.stringify({type:'openPath',path:${JSON.stringify(fixture)}}))`,
    );
    await wait(
      `document.querySelectorAll('.tab').length===2 && document.title.includes('fixture.md')`,
      "native file open",
    );
    await menu("View", "view-source");
    await wait(`document.querySelector('#document').dataset.mode==='source'`);
    const saved =
      '# Saved by menu\n\nनमस्ते 😀\n\n<audio src="Todos.assets\\Krishna Madam 2021-09-16 09-50-50.wav" />\n- [ ] Krishna School Mam\n  - [x] TC format for counter signature\n  - [ ] Attendance Sheet\n<audio src="Todos.assets\\Krishna Madam 2021-09-16 09-50-50.wav" />\n- [ ] Kapil Goyal Mathura\n  - [ ] Manually Enter Fees Fine Amount\n  - [x] Manually Adjust Headwise Fees\n\nAfter audio.\n';
    await input(saved);
    await menu("File", "save");
    for (let attempt = 0; attempt < 100 && fs.readFileSync(fixture, "utf8") !== saved; attempt++) await delay(50);
    await wait(
      `document.querySelector('#save-state').textContent==='Saved'`,
      "save acknowledgement",
    );
    assert.equal(fs.readFileSync(fixture, "utf8"), saved);
    await click("#toggle-focus");
    await wait(`!document.querySelector('#files-panel').hidden`);
    const screenshot = await call("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(
      path.join(artifacts, "source-dark.png"),
      Buffer.from(screenshot.data, "base64"),
    );
    await menu("View", "view-live");
    await wait(`document.querySelector('#live-document h1')?.textContent==='Saved by menu'`);
    await wait(`document.querySelector('#document').dataset.mode==='live' && !document.querySelector('#live-pane').hidden`);
    // Ending an unchanged edit must restore the block and preserve other blocks.
    await click('.live-block[data-block="0"]');
    await call("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape" });
    await call("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape" });
    await wait(`!document.querySelector('.block-editor') && document.querySelector('#live-document h1')?.textContent==='Saved by menu'`,
      "unchanged Live edit finishes and renders again");
    await wait(
      `(()=>{const a=document.querySelector('#live-document audio');return Boolean(a && a.readyState>=1 && a.duration>0 && document.querySelector('#live-document').textContent.includes('After audio.'));})()`,
      "local audio element loads and decodes",
    );
    const todoLayout = await evaluate(`(()=>{
      const topLi = document.querySelector('#live-document .live-block > ul > li');
      const topBox = topLi.querySelector('input[type="checkbox"]').getBoundingClientRect();
      const nestedUl = topLi.querySelector('ul');
      const nestedBox = nestedUl.querySelector('input[type="checkbox"]').getBoundingClientRect();
      return { mode: document.querySelector('#document').dataset.mode, paneHidden: document.querySelector('#live-pane').hidden, topBottom: topBox.bottom, topLeft: topBox.left, nestedTop: nestedUl.getBoundingClientRect().top, nestedLeft: nestedBox.left };
    })()`);
    assert(todoLayout.nestedTop >= todoLayout.topBottom - 2, `nested todo list must stack below parent: ${JSON.stringify(todoLayout)}`);
    assert(todoLayout.nestedLeft > todoLayout.topLeft + 10, `nested todo list must be indented: ${JSON.stringify(todoLayout)}`);
    await click('#live-document li ul li:nth-child(2) input[type="checkbox"]');
    await menu("File", "save");
    for (let attempt = 0; attempt < 100 && !fs.readFileSync(fixture, "utf8").includes('  - [x] Attendance Sheet'); attempt++) await delay(50);
    await wait(`document.querySelector('#save-state').textContent==='Saved'`);
    assert(fs.readFileSync(fixture, "utf8").includes('  - [x] Attendance Sheet'),
      "clicking nested todo checkbox toggles saved Markdown");
    await menu("View", "toggle-theme");
    await wait(`document.documentElement.dataset.color==='light'`);
    const light = await call("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(path.join(artifacts, "live-light.png"), Buffer.from(light.data, "base64"));
    assert.equal(await evaluate(`document.querySelector('.brand')`), null);
    await menu("Help", "about");
    await wait(`document.querySelector('#about-dialog')?.open`, "Help > About Markora modal");
    await click('#about-dialog [data-close-dialog]');
    await wait(`!document.querySelector('#about-dialog')?.open`, "close About modal");
    await menu("File", "export");
    await wait(`document.querySelector('#export-dialog')?.open`, "File > Export modal");
    await click('input[name="export-format"][value="pdf"]');
    await wait(`document.querySelector('#export-html-options')?.hidden`, "PDF hides HTML options");
    await click('input[name="export-format"][value="html"]');
    await wait(`!document.querySelector('#export-html-options')?.hidden`, "HTML shows HTML options");
    await click('#export-dialog [data-close-dialog]');
    await wait(`!document.querySelector('#export-dialog')?.open`, "close Export modal");
    await call("Emulation.setEmulatedMedia", { media: "print" });
    const printBox = await evaluate(
      `(() => { const p = document.querySelector('#preview-pane'); const r = p.getBoundingClientRect(); return { display: getComputedStyle(p).display, h: r.height }; })()`,
    );
    assert.equal(printBox.display, "block");
    assert(printBox.h > 20, "print preview must render content");
    await call("Emulation.setEmulatedMedia", { media: "" });

    // Create a minimal valid PDF and verify both relative-link opening and inline PDF viewing.
    const pdfPath = path.join(artifacts, "smoke-guide.pdf");
    const pdfStream = "BT /F1 18 Tf 72 720 Td (Markora Smoke PDF) Tj ET";
    const pdfObjs = [
      "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
      "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
      "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
      `4 0 obj\n<< /Length ${pdfStream.length} >>\nstream\n${pdfStream}\nendstream\nendobj\n`,
      "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
    ];
    let pdfText = "%PDF-1.4\n";
    const pdfOffsets = [];
    for (const obj of pdfObjs) {
      pdfOffsets.push(Buffer.byteLength(pdfText, "utf8"));
      pdfText += obj;
    }
    const xrefOffset = Buffer.byteLength(pdfText, "utf8");
    pdfText += `xref\n0 ${pdfObjs.length + 1}\n0000000000 65535 f \n`;
    for (const off of pdfOffsets) pdfText += `${String(off).padStart(10, "0")} 00000 n \n`;
    pdfText += `trailer\n<< /Size ${pdfObjs.length + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
    fs.writeFileSync(pdfPath, pdfText);

    const mdWithPdfLink = path.join(artifacts, "with-pdf-link.md");
    fs.writeFileSync(mdWithPdfLink, "# Links\n\n[Open Smoke PDF](smoke-guide.pdf)\n");
    await evaluate(`window.ipc.postMessage(JSON.stringify({type:'openPath',path:${JSON.stringify(mdWithPdfLink)}}))`);
    await wait(`document.title.includes('with-pdf-link.md') && document.querySelector('#live-document a[href="smoke-guide.pdf"]')`, "open md with relative pdf link");
    await click('#live-document a[href="smoke-guide.pdf"]');
    await wait(
      `document.title.includes('smoke-guide.pdf') && document.querySelector('#document').dataset.mode==='pdf' && document.querySelector('#pdf-frame').src.includes('/pdf/')`,
      "relative pdf link opens inline PDF viewer",
    );
    const pdfBox = await evaluate(`(() => { const r = document.querySelector('#pdf-frame').getBoundingClientRect(); return { w: r.width, h: r.height, hidden: document.querySelector('#pdf-pane').hidden }; })()`);
    assert.equal(pdfBox.hidden, false);
    assert(pdfBox.w > 200 && pdfBox.h > 200, "PDF iframe must fill workspace");
    console.log("PASS relative .pdf link opens inline WebView2 PDF viewer");
    const readingScreenshot = await require("./check-reading.cjs").pdf(client, app.pid);
    fs.writeFileSync(path.join(artifacts, "pdf-reading.png"), Buffer.from(readingScreenshot.data, "base64"));

    const largePath = path.join(artifacts, "large.md");
    let largeMd = "";
    const sections = 8000;
    for (let i = 0; i < sections; i++) {
      largeMd += `# Section ${i}\n\nParagraph ${i} with **bold** text and ` + "lorem ipsum ".repeat(24) + "\n\n";
    }
    fs.writeFileSync(largePath, largeMd);
    const openStart = Date.now();
    await evaluate(
      `window.ipc.postMessage(JSON.stringify({type:'openPath',path:${JSON.stringify(largePath)}}))`,
    );
    await wait(
      `document.title.includes('large.md') && document.querySelectorAll('.live-block').length>=60`,
      "large file initial render",
    );
    const openMs = Date.now() - openStart;
    console.log(`PASS large Live initial view (${openMs} ms)`);
    // Start editing before the frame queue finishes, then finish without changes.
    await click('.live-block[data-block="0"]');
    await call("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape" });
    await call("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape" });
    await wait(`document.querySelectorAll('#live-document .live-block').length===${sections * 2}`,
      "progressive rendering resumes after finishing an unchanged edit", 15000);
    await click("#outline button:last-child");
    await wait(
      `document.querySelector('.block-editor')?.value.includes('Section ${sections - 1}')`,
      "deep outline jump on large file",
    );
    await menu("View", "view-source");
    await wait(`document.querySelector('#document').dataset.mode==='source'`);
    await menu("View", "view-live");
    await wait(`document.querySelector('#document').dataset.mode==='live'`);
    await wait(`document.querySelectorAll('#live-document .live-block').length===${sections * 2}`,
      "all large-file blocks survive Source/Live roundtrip", 15000);
    console.log("PASS large Live completion, unchanged edit, outline jump and Source/Live roundtrip");
    await menu("View", "view-source");
    await wait(`document.querySelector('#document').dataset.mode==='source'`);
    const sourceLargePath = path.join(artifacts, "large-source.md");
    fs.writeFileSync(sourceLargePath, largeMd);
    const sourceOpenStart = Date.now();
    await evaluate(`window.ipc.postMessage(JSON.stringify({type:'openPath',path:${JSON.stringify(sourceLargePath)}}))`);
    await wait(`document.title.includes('large-source.md') && document.querySelector('#source-editor').value.length===${largeMd.length}`,
      "large source file opens with its outline and complete text");
    const sourceOpenMs = Date.now() - sourceOpenStart;
    console.log(`PASS large Source open (${sourceOpenMs} ms)`);
    const changedLarge = largeMd + "\nLast **unsaved** paragraph.\n";
    await input(changedLarge);
    await evaluate(`(()=>{const t=document.querySelector('#source-editor');t.focus();t.setSelectionRange(16000,16007);t.scrollTop=3000;window.readingScroll=t.scrollTop;})()`);
    await menu("View", "toggle-reading");
    await wait(`document.body.classList.contains('reading-mode') && document.querySelector('#preview-document').textContent.endsWith('Last unsaved paragraph.\\n')`,
      "reading prepares the latest unsaved deferred Source document", 15000);
    await click('#exit-reading');
    await wait(`!document.body.classList.contains('reading-mode')`, "large Source reading exit");
    await wait(`document.querySelector('#source-editor').selectionStart===16000 && document.querySelector('#source-editor').scrollTop===window.readingScroll`, "large Source selection and scroll restored");
    // A state response arriving while the large draft is debounced must not lose the edit.
    await menu("View", "view-split");
    await wait(`[...document.querySelectorAll('#preview-document p')].at(-1)?.textContent==='Last unsaved paragraph.'`,
      "deferred Source render and immediate mode change preserve the newest draft");
    assert.equal(await evaluate(`document.querySelector('#source-editor').value`), changedLarge);
    console.log("PASS large debounced source draft reaches Split preview");
    await menu("View", "view-source");
    await wait(`document.querySelector('#document').dataset.mode==='source'`);
    // Observe the production acknowledgement; send an obsolete token to avoid a print dialog.
    await evaluate(`(()=>{
      const stringify=JSON.stringify;
      JSON.stringify=function(value,...args){
        if(value?.type==='printReady'){
          window.printAcknowledgement={token:value.token,revision:value.draft?.revision};
          value={...value,token:0};
        }
        return stringify.call(this,value,...args);
      };
    })()`);
    await input(changedLarge + "\nLatest print **draft**.\n");
    await menu("File", "print");
    await wait(`window.printAcknowledgement && [...document.querySelectorAll('#preview-document p')].at(-1)?.textContent==='Latest print draft.'`,
      "native print waits for the newest deferred source preview");
    assert.deepEqual(client.errors, []);
    console.log(`PASS real file open, immediate edit+Save, ${Math.round(largeMd.length / 1024)} KiB file: Live ${openMs} ms / Source ${sourceOpenMs} ms, render completion and clean runtime`);
    console.log(`Artifacts: ${artifacts}`);
  } catch (error) {
    if (client) {
      try {
        fs.writeFileSync(
          path.join(artifacts, "failure-state.json"),
          JSON.stringify(
            await client.evaluate(
              `({html:document.body.innerText,errors:document.querySelector('#error-text')?.textContent,mode:document.querySelector('#document')?.dataset.mode,blocks:document.querySelectorAll('#live-document .live-block').length,editing:!!document.querySelector('.block-editor')})`,
            ),
            null,
            2,
          ),
        );
      } catch {}
      try {
        const result = await client.call("Page.captureScreenshot", { format: "png" });
        fs.writeFileSync(path.join(artifacts, "failure.png"), Buffer.from(result.data, "base64"));
      } catch {}
    }
    console.error(`Artifacts: ${artifacts}`);
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
