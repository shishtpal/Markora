const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
function moduleSource(name, symbol) {
  const source = fs.readFileSync(path.join(__dirname, '../assets/ui/src', name + '.js'), 'utf8');
  return new vm.Script(source.replace(/^export /gm, '') + `\n${symbol}`).runInNewContext({});
}
const createImageEdits = moduleSource('editor-images', 'createImageEdits');
function editor(markdown, mode = 'source') {
  const doc = { id: 7, revision: 4, markdown, kind: 'markdown', viewMode: mode, rendered: { blocks: [] } };
  const source = { selectionStart: 0, selectionEnd: 0, setSelectionRange(a,b) { this.selectionStart=a; this.selectionEnd=b; }, focus() {} };
  const edits = [], live = { resets: 0, selection: () => null, reset() { this.resets++; } };
  const api = createImageEdits(() => doc, () => false, live, source, (text, record, immediate) => {
    edits.push({ text, record, immediate }); doc.markdown = text; doc.revision++;
  }, () => {});
  return { doc, source, live, api, edits };
}
test('image selection uses UTF-16 offsets and escapes paths and alt text', () => {
  const e = editor('A 😀 B');
  e.source.selectionStart = 2; e.source.selectionEnd = 4;
  const anchor = e.api.capture();
  assert.equal(anchor.start, 2);
  assert(e.api.insert(anchor, [{ relativePath: 'My #नोट.assets/image.png', alt: 'a[b]\\c' }]));
  assert.equal(e.doc.markdown, 'A ![a\\[b\\]\\\\c](My%20%23%E0%A4%A8%E0%A5%8B%E0%A4%9F.assets/image.png) B');
  assert.equal(e.edits.length, 1); assert(e.edits[0].record && e.edits[0].immediate);
});
test('stale revisions, other tabs and reading/PDF modes do not accept image edits', () => {
  const e = editor('original'), anchor = e.api.capture();
  e.doc.revision++;
  assert(!e.api.insert(anchor, [{ relativePath: 'image.png', alt: 'x' }]));
  e.doc.revision--; e.doc.id++;
  assert(!e.api.insert(anchor, []));
  assert.equal(e.edits.length, 0);
  e.doc.kind = 'pdf'; assert.equal(e.api.capture(), null);
});
test('unbalanced filename parentheses cannot terminate a Markdown image link', () => {
  const e = editor('');
  assert(e.api.insert(e.api.capture(), [{ relativePath: 'notes).assets/image.png', alt: 'Image' }]));
  assert.equal(e.doc.markdown, '![Image](notes%29.assets/image.png)');
});
test('Live block selection uses document offsets and batches images into one edit', () => {
  const e = editor('first\n\nsecond', 'live');
  e.live.selection = () => ({ start: 7, end: 13 });
  assert(e.api.insert(e.api.capture(), [{ relativePath: 'a.png', alt: 'a' }, { relativePath: 'b.png', alt: 'b' }]));
  assert.equal(e.doc.markdown, 'first\n\n![a](a.png)\n![b](b.png)');
  assert.equal(e.edits.length, 1); assert.equal(e.live.resets, 1);
});
test('rendered Live drops insert a separate paragraph after the target block', () => {
  const e = editor('first\n\nsecond', 'live');
  e.doc.rendered.blocks = [{ start: 0, end: 5 }];
  const node = { dataset: { block: '0' } };
  const anchor = e.api.capture({ closest: () => node });
  assert(e.api.insert(anchor, [{ relativePath: 'a.png', alt: 'a' }]));
  assert.equal(e.doc.markdown, 'first\n\n![a](a.png)\n\nsecond');
});
test('preview normalizes encoded segments once, retaining literal percent escapes', () => {
  const resolve = moduleSource('dom', 'resolveMedia');
  for (const [src, expected] of [['My%20notes.assets/a.png', 'My%20notes.assets/a.png'], ['नोट.assets/a%2520b.png', '%E0%A4%A8%E0%A5%8B%E0%A4%9F.assets/a%2520b.png']]) {
    let result;
    const item = { getAttribute: (name) => name === 'src' ? src : null, setAttribute: (_name, value) => { result = value; } };
    resolve({ querySelectorAll: (query) => query.includes('img') ? [item] : [] }, 7);
    assert.equal(result, 'http://markora.localhost/doc/7/relative/' + encodeURIComponent(decodeURIComponent(expected)));
  }
});
