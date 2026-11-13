// Real Rust + WebView2: clipboard image, native OLE file drop, async retry and Save As.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const { spawn, spawnSync } = require('node:child_process');
const { connect, delay } = require('./cdp-client.cjs');
const root = path.resolve(__dirname, '..');

async function main() {
  const domOnly = process.argv.includes('--dom');
  if (process.platform !== 'win32') throw Error('Requires Windows and WebView2');
  const artifacts = path.join(root, 'target/image-smoke', String(Date.now()));
  fs.mkdirSync(artifacts, { recursive: true });
  const note = path.join(artifacts, 'My notes # नमस्ते.md');
  fs.writeFileSync(note, 'Before 😀 After');
  const png = fs.readFileSync(path.join(root, 'assets/markora.png'));
  const imagePath = path.join(artifacts, 'Photo # नमस्ते.png');
  fs.writeFileSync(imagePath, png);
  const server = net.createServer();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  const app = spawn(path.join(root, 'target/debug/markora.exe'), [], {
    cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, MARKORA_DATA_DIR: path.join(artifacts, 'settings'), WEBVIEW2_USER_DATA_FOLDER: path.join(artifacts, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=* --disable-background-timer-throttling --disable-renderer-backgrounding` },
  });
  let client, logs = '', clipboard;
  app.stdout.on('data', (c) => { logs += c; }); app.stderr.on('data', (c) => { logs += c; });
  const helper = (action, args = []) => {
    const result = spawnSync('powershell.exe', ['-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'tools/image-input.ps1'), '-Action', action, '-ProcessId', String(app.pid), ...args], { cwd: root, windowsHide: true, encoding: 'utf8', timeout: 20000 });
    if (result.error || result.status) throw Error(result.error?.message || result.stdout + result.stderr);
  };
  try {
    client = await connect(port);
    const { evaluate, wait, call } = client;
    const send = (message) => evaluate(`window.ipc.postMessage(${JSON.stringify(JSON.stringify(message))})`);
    const cmd = (command) => evaluate(`document.querySelector(${JSON.stringify('[data-command="' + command + '"]')}).click()`);
    const click = (selector) => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    const text = () => evaluate(`document.querySelector('#source-editor').value`);
    const edit = (value) => evaluate(`(() => { const t=document.querySelector('#source-editor'); t.value=${JSON.stringify(value)}; t.dispatchEvent(new Event('input', {bubbles:true})); })()`);
    const position = (start, end = start) => evaluate(`(() => {const t=document.querySelector('#source-editor'); t.focus(); t.setSelectionRange(${start},${end}); })()`);
    const links = (value) => [...value.matchAll(/!\[[^\n]*?\]\(([^)]+)\)/g)].map((m) => m[1]);
    const pasted = () => wait(`document.querySelector('#source-editor').value.includes('/image-') && document.querySelector('#image-import-note').hidden`, 'image inserted', 12000);
    async function pasteSynthetic(bytes = png, name = 'photo.png') {
      await evaluate(`(() => { const bytes=Uint8Array.from(atob(${JSON.stringify(bytes.toString('base64'))}), c=>c.charCodeAt(0)); const data=new DataTransfer(); data.items.add(new File([bytes], ${JSON.stringify(name)}, {type:'image/png'})); document.querySelector('#source-editor').dispatchEvent(new ClipboardEvent('paste',{clipboardData:data,bubbles:true,cancelable:true})); })()`);
    }
    await wait(`document.querySelector('#connection')?.classList.contains('connected')`, 'connected', 10000);
    await evaluate(`(() => {window.imageMessages=[];window.imageStates=[];const post=window.ipc.postMessage.bind(window.ipc);window.ipc.postMessage=(raw)=>{const m=JSON.parse(raw);if(m.type==='image')window.imageMessages.push(m);post(raw);};const receive=window.markoraReceive;window.markoraReceive=(m)=>{if(m.snapshot)window.imageStates.push({status:m.snapshot.status,image:m.snapshot.imageImport});receive(m);};})()`);
    await send({ type: 'openPath', path: note });
    await wait(`document.title.includes('My notes')`, 'note open');
    await cmd('view-source');
    await wait(`document.querySelector('#document').dataset.mode==='source'`, 'Source mode');
    await position(10, 15);
    if (domOnly) {
      await pasteSynthetic();
      await pasted();
    } else {
    const restore = path.join(artifacts, 'restore-clipboard');
    clipboard = spawn('powershell.exe', ['-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'tools/image-input.ps1'), '-Action', 'Clipboard', '-ImagePath', imagePath, '-RestoreSignal', restore], { windowsHide: true });
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('Clipboard helper timeout')), 10000);
      clipboard.stdout.on('data', (c) => { if (String(c).includes('READY')) { clearTimeout(timer); resolve(); } });
      clipboard.stderr.on('data', (c) => { clearTimeout(timer); reject(Error(String(c))); });
    });
    await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'v', code: 'KeyV', windowsVirtualKeyCode: 86, modifiers: 2 });
    await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'v', code: 'KeyV', windowsVirtualKeyCode: 86, modifiers: 2 });
    await pasted();
    fs.writeFileSync(restore, 'restore');
    }
    let result = await text();
    assert.equal(links(result).length, 1);
    assert(result.startsWith('Before 😀 ')); assert(!result.includes('After'));
    const clipboardLink = links(result)[0];
    assert(fs.existsSync(path.join(artifacts, decodeURIComponent(clipboardLink))));
    await cmd('undo'); await wait(`document.querySelector('#source-editor').value==='Before 😀 After'`, 'single undo');
    await cmd('redo'); await pasted();
    console.log(`PASS ${domOnly ? 'DOM clipboard event' : 'real OS clipboard screenshot'}, UTF-16 selection, relative file, Undo/Redo`);

    await cmd('view-split');
    await wait(`document.querySelector('#preview-document img')?.naturalWidth>0`, 'image preview with encoded Unicode/space path');
    const shot = await call('Page.captureScreenshot'); fs.writeFileSync(path.join(artifacts, 'image-preview.png'), Buffer.from(shot.data, 'base64'));
    await cmd('view-source');
    await position(result.length);
    const box = await evaluate(`(() => {const r=document.querySelector('#source-editor').getBoundingClientRect(); return {x:Math.round((r.left+r.width/2)*devicePixelRatio),y:Math.round((r.top+80)*devicePixelRatio)};})()`);
    if (domOnly) await send({ type:'nativeDrop', paths:[imagePath], position:[box.x,box.y] });
    else helper('Drop', ['-ImagePath', imagePath, '-X', String(box.x), '-Y', String(box.y)]);
    await wait(`document.querySelector('#source-editor').value.split('.assets/image-').length===3`, 'real OS drop', 15000);
    const dropped = await text();
    assert.equal(links(dropped).length, 2);
    console.log(`PASS ${domOnly ? 'native-path drop routing' : 'native OLE file drop'} inserts exactly once`);

    // Pad a valid PNG to span multiple acknowledged transfer chunks.
    await edit('Chunked '); await position(8);
    await pasteSynthetic(Buffer.concat([png, Buffer.alloc(700000)])); await pasted();
    const chunked = await text();
    assert.equal(links(chunked).length, 1);
    assert.equal(fs.statSync(path.join(artifacts, decodeURIComponent(links(chunked)[0]))).size, png.length + 700000);
    await cmd('save');
    await wait(`!document.title.startsWith('•')`, 'saved');
    assert.equal(fs.readFileSync(note, 'utf8'), chunked);
    console.log('PASS multi-chunk byte transfer and saved Markdown');

    const other = path.join(artifacts, 'copy'); fs.mkdirSync(other);
    if (!domOnly) {
    await cmd('save-as'); helper('Save', ['-SavePath', path.join(other, 'renamed.md')]);
    await wait(`document.title.includes('renamed.md')`, 'Save As completion', 10000);
    const renamed = fs.readFileSync(path.join(other, 'renamed.md'), 'utf8');
    assert(renamed.includes('renamed.assets/'));
    assert(fs.existsSync(path.join(other, decodeURIComponent(links(renamed)[0]))));
    }
    await cmd('view-live'); await wait(`document.querySelector('#live-document img')?.naturalWidth>0`, 'relocated image preview');
    await click('#toggle-edit');
    await click('#live-document [data-block]');
    await wait(`document.querySelector('.block-editor')`, 'Live editing');
    await evaluate(`(() => {const t=document.querySelector('.block-editor');t.focus();t.setSelectionRange(0,0);})()`);
    await evaluate(`(() => {const data=new DataTransfer();data.items.add(new File([Uint8Array.from(atob(${JSON.stringify(png.toString('base64'))}),c=>c.charCodeAt(0))],'live.png',{type:'image/png'}));document.querySelector('.block-editor').dispatchEvent(new ClipboardEvent('paste',{clipboardData:data,bubbles:true,cancelable:true}));})()`);
    await wait(`document.querySelectorAll('#live-document img').length===2`, 'Live image insertion');
    console.log(`PASS ${domOnly ? '' : 'cross-directory Save As and '}Live block insertion`);

    // Delay completion at the UI transport boundary, then edit to invalidate the captured range.
    await cmd('view-source'); await edit('Latest '); await position(7);
    await evaluate(`window.realImageReceive=window.markoraReceive;window.markoraReceive=(m)=>{if(m.snapshot?.imageImport?.status==='complete')window.heldImage=m;else window.realImageReceive(m);};`);
    await pasteSynthetic();
    await wait(`!!window.heldImage`, 'completed background import');
    await edit('Newest draft');
    await evaluate(`window.markoraReceive=window.realImageReceive;window.markoraReceive(window.heldImage);`);
    await wait(`!document.querySelector('#image-import-retry').hidden`, 'stale insertion retry');
    assert.equal(await text(), 'Newest draft');
    await position(12); await click('#image-import-retry'); await pasted();
    assert((await text()).startsWith('Newest draft'));
    console.log('PASS changed-document completion offers retry without overwriting typing');

    if (!domOnly) {
    await cmd('new-tab'); await cmd('view-source');
    await pasteSynthetic(); helper('Cancel');
    await wait(`document.querySelector('#error-text').textContent.includes('cancelled')`, 'untitled save cancelled');
    assert.equal(await text(), '');
    await pasteSynthetic(); const savedNote = path.join(artifacts, 'untitled-saved.md');
    helper('Save', ['-SavePath', savedNote]); await pasted();
    assert(fs.existsSync(savedNote));
    assert(fs.readdirSync(path.join(artifacts, 'untitled-saved.assets')).length === 1);
    assert(!fs.readFileSync(savedNote, 'utf8').includes('image-'), 'imported link remains a dirty draft until Save');
    await cmd('save'); await wait(`!document.title.startsWith('•')`, 'untitled import saved');
    assert(fs.readFileSync(savedNote, 'utf8').includes('image-'));
    console.log('PASS untitled Save/Cancel creates assets only after a destination exists');
    }

    // Settings are exercised through the actual frontend and persisted Rust preferences.
    async function assetSetting(mode, custom = '') {
      await cmd('settings');
      await evaluate(`(() => {const mode=document.querySelector('#asset-mode');mode.value=${JSON.stringify(mode)};mode.dispatchEvent(new Event('change'));})()`);
      await wait(`document.querySelector('#asset-mode').value===${JSON.stringify(mode)}`, 'asset mode');
      if (mode === 'custom') await evaluate(`(() => {const input=document.querySelector('#asset-custom-path');input.value=${JSON.stringify(custom)};input.dispatchEvent(new Event('change'));})()`);
      if (mode === 'custom') {
        await delay(150);
        const settingsShot = await call('Page.captureScreenshot');
        fs.writeFileSync(path.join(artifacts, 'asset-settings.png'), Buffer.from(settingsShot.data, 'base64'));
      }
      await delay(150);
      await click('#close-settings');
      const prefs = JSON.parse(fs.readFileSync(path.join(artifacts, 'settings/prefs.json'), 'utf8'));
      assert.equal(prefs.assetMode, mode);
      if (mode === 'custom') assert.equal(prefs.assetCustomPath, custom);
    }
    await cmd('view-source');
    await assetSetting('shared'); await edit('Shared '); await position(7); await pasteSynthetic(); await pasted();
    assert((await text()).includes('data.assets/'));
    const sharedFiles = fs.readdirSync(path.join(artifacts, 'data.assets'));
    const second = path.join(artifacts, 'second.md'); fs.writeFileSync(second, 'Shared again ');
    await send({type:'openPath',path:second}); await wait(`document.title.includes('second.md')`, 'second note');
    await position(13); await pasteSynthetic(); await pasted();
    assert.deepEqual(fs.readdirSync(path.join(artifacts, 'data.assets')), sharedFiles);
    const nested = path.join(artifacts,'notes'); fs.mkdirSync(nested);
    const customNote = path.join(nested,'custom.md'); fs.writeFileSync(customNote, 'Custom ');
    await send({type:'openPath',path:customNote}); await wait(`document.title.includes('custom.md')`, 'custom note');
    const customFolder = path.join(artifacts,'global-images');
    await assetSetting('custom',customFolder); await position(7); await pasteSynthetic(); await pasted();
    assert((await text()).includes('../global-images/'));
    await cmd('view-split'); await wait(`document.querySelector('#preview-document img')?.naturalWidth>0`, 'custom folder preview');
    await assetSetting('file');
    await evaluate(`document.querySelector('#preview-document img').src += '?refresh=1'`);
    await wait(`document.querySelector('#preview-document img')?.naturalWidth>0`, 'existing custom image survives settings change');
    assert(fs.existsSync(path.join(artifacts,'settings/imported-images.json')));
    console.log('PASS settings persist per-file/shared/custom modes; shared files deduplicate; external custom images remain viewable after settings change');
    assert.deepEqual(client.errors, []);
    console.log(`Artifacts: ${artifacts}`);
  } catch (error) {
    if (client) {
      try { const shot=await client.call('Page.captureScreenshot'); fs.writeFileSync(path.join(artifacts,'failure.png'),Buffer.from(shot.data,'base64')); fs.writeFileSync(path.join(artifacts,'failure.json'),JSON.stringify(await client.evaluate(`({text:document.body.innerText,mode:document.querySelector('#document').dataset.mode,messages:window.imageMessages?.map(m=>({...m,request:{...m.request,data:m.request.data?.slice(0,30)}})),states:window.imageStates})`),null,2)); } catch {}
    }
    console.error(`Artifacts: ${artifacts}`); throw error;
  } finally {
    fs.writeFileSync(path.join(artifacts,'restore-clipboard'), 'restore');
    if (clipboard && clipboard.exitCode === null) await Promise.race([new Promise((resolve)=>clipboard.on('exit',resolve)), delay(3000)]);
    fs.writeFileSync(path.join(artifacts,'runtime.log'), logs);
    client?.close();
    if (app.exitCode === null) spawnSync('taskkill',['/pid',String(app.pid),'/T','/F'],{windowsHide:true});
  }
}
main().catch((error)=>{ console.error(error); process.exitCode=1; });
