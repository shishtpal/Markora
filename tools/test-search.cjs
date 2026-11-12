// Semantic regressions against the actual worker engine, without a mirrored implementation.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { test } = require("node:test");
const source = fs.readFileSync(path.join(__dirname, "../assets/ui/src/search-engine.js"), "utf8");
const engine = import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
const run = async (request) => (await engine).searchDocument(request);

test("literal searches escape regex metacharacters and preserve original UTF-16 offsets", async () => {
  assert.deepEqual((await run({ text: "İx 😀 [a+b] [aXb]", query: "[a+b]" })).hits, [
    { start: 6, length: 5 },
  ]);
  assert.deepEqual((await run({ text: "İx 😀 X", query: "x" })).hits, [
    { start: 1, length: 1 },
    { start: 6, length: 1 },
  ]);
});

test("case-sensitive and Unicode whole-word options compose", async () => {
  const request = { text: "cat Cat scatter cat_ cat", query: "cat" };
  assert.equal((await run(request)).hits.length, 5);
  assert.equal((await run({ ...request, caseSensitive: true })).hits.length, 4);
  assert.equal((await run({ ...request, wholeWord: true })).hits.length, 3);
  assert.equal((await run({ ...request, wholeWord: true, caseSensitive: true })).hits.length, 2);
  assert.equal(
    (await run({ text: "नमस्ते नमस्तेपन 😀नमस्ते_ नमस्ते", query: "नमस्ते", wholeWord: true })).hits.length,
    2,
  );
  assert.equal(
    (await run({ text: "café caféine café_ café", query: "café", wholeWord: true })).hits.length,
    2,
  );
});
test("regex uses multiline anchors, lookarounds and actual match lengths", async () => {
  assert.deepEqual(
    (await run({ text: "x=1\nx=22\n😀 x=333", query: "(?<=x=)\\d+", regex: true })).hits,
    [
      { start: 2, length: 1 },
      { start: 6, length: 2 },
      { start: 14, length: 3 },
    ],
  );
  assert.equal((await run({ text: "x\nx\ny", query: "^x$", regex: true })).hits.length, 2);
});
test("literal replacement treats dollar sequences as literal text", async () => {
  const result = await run({
    text: "cat cat",
    query: "cat",
    replacement: "$1 $$ $&",
    operation: "replaceAll",
  });
  assert.equal(result.text, "$1 $$ $& $1 $$ $&");
  assert.equal(result.replaced, 2);
});
test("replace current changes only its selected match and supports deletion", async () => {
  assert.equal(
    (await run({ text: "a a a", query: "a", replacement: "b", operation: "replace", index: 1 }))
      .text,
    "a b a",
  );
  assert.equal(
    (await run({ text: "abc abc", query: "b", replacement: "", operation: "replaceAll" })).text,
    "ac ac",
  );
});
test("regex capture substitutions follow JavaScript replacement semantics", async () => {
  const examples = [
    ["ab a", "(a)(b)?", "$1-$2-$01-$10-$0-$99-$$-$&"],
    ["ab a", "(?<one>a)(?<two>b)?", "$<two>:$<one>:$<missing>"],
    ["ab a", "(a)", "$<absent>:$`:$'"],
  ];
  for (const [text, query, replacement] of examples) {
    const result = await run({ text, query, replacement, regex: true, operation: "replaceAll" });
    assert.equal(result.text, text.replace(new RegExp(query, "gimu"), replacement));
  }
});
test("Unicode zero-length matches terminate and empty replacement advances", async () => {
  const request = { text: "😀", query: "(?:)", regex: true };
  assert.deepEqual((await run(request)).hits, [
    { start: 0, length: 0 },
    { start: 2, length: 0 },
  ]);
  assert.equal((await run({ ...request, replacement: "|", operation: "replaceAll" })).text, "|😀|");
  const result = await run({ ...request, replacement: "", operation: "replace" });
  assert.equal(result.text, "😀");
  assert.equal(result.anchor, 2);
});
test("empty queries and no matches never produce replacement text", async () => {
  assert.equal(
    (await run({ text: "original", query: "", operation: "replaceAll" })).text,
    undefined,
  );
  assert.equal(
    (await run({ text: "original", query: "missing", operation: "replaceAll" })).text,
    undefined,
  );
});
test("invalid expressions report errors instead of modifying a document", async () => {
  await assert.rejects(
    run({ text: "original", query: "[", regex: true, operation: "replaceAll" }),
    SyntaxError,
  );
});
test("match limits prevent partial replace-all operations", async () => {
  const request = { text: "a".repeat(100_001), query: "a" };
  const found = await run(request);
  assert.equal(found.hits.length, 100_000);
  assert.equal(found.limited, true);
  await assert.rejects(run({ ...request, operation: "replaceAll", replacement: "b" }), /Refine/);
  assert.equal((await run({ text: "a".repeat(100_000), query: "a" })).limited, false);
});
test("replacement output limit rejects excessive expansion", async () => {
  await assert.rejects(
    run({
      text: "a".repeat(1000),
      query: "a",
      replacement: "b".repeat(50_000),
      operation: "replaceAll",
    }),
    /exceeds/,
  );
});
