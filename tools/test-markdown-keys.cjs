const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const context = vm.createContext({ Event });
for (const name of ["table-editing", "markdown-keys"]) {
  const source = fs.readFileSync(path.join(__dirname, "../assets/ui/src", name + ".js"), "utf8");
  vm.runInContext(source.replace(/^import .*;\r?\n/gm, "").replace(/^export /gm, ""), context);
}
const { markdownKeyEdit: edit, handleMarkdownKey: handle } = context;
const enter = (text) => edit(text, text.length, text.length, "Enter");
test("Enter continues bullet, numbered, nested, quoted and task lists", () => {
  for (const [input, expected] of [
    ["- hello", "- hello\n- "],
    ["+ hello", "+ hello\n+ "],
    ["* hello", "* hello\n* "],
    ["9. hello", "9. hello\n10. "],
    ["3) hello", "3) hello\n4) "],
    ["  - nested", "  - nested\n  - "],
    ["> - quoted", "> - quoted\n> - "],
    ["- [x] done", "- [x] done\n- [ ] "],
    ["12. [X] done", "12. [X] done\n13. [ ] "],
    ["- [ ] todo", "- [ ] todo\n- [ ] "],
    ["- 😀 नमस्ते", "- 😀 नमस्ते\n- "],
  ]) {
    const result = enter(input);
    assert.equal(result.value, expected);
    assert.equal(result.start, expected.length);
    assert.equal(result.end, expected.length);
  }
});
test("Enter exits empty list/task items without consuming the next line", () => {
  for (const input of ["- ", "1. ", "- [ ] ", "- [x]", "+ [ ]"])
    assert.equal(enter(input).value, "");
  assert.equal(enter("  - ").value, "  ");
  assert.equal(enter("> - [ ] ").value, "> ");
  const result = edit("- \nnext", 2, 2, "Enter");
  assert.equal(result.value, "\nnext");
  assert.equal(result.start, 0);
});
test("Enter splits an item or replaces selected content using UTF-16 offsets", () => {
  const input = "- 😀 hello world";
  const start = input.indexOf("hello"),
    end = input.indexOf("world");
  assert.equal(edit(input, start, end, "Enter").value, "- 😀 \n- world");
  assert.equal(edit("- hello", 1, 1, "Enter"), null);
  assert.equal(edit("- hello", 7, 7, "Enter", true), null);
});
test("fences, thematic breaks and ordinary text do not become lists or tables", () => {
  for (const text of [
    "```md\n- code",
    "~~~~\n- code",
    "- ```\n  - code",
    "> ```\n> - code",
    "* * *",
    "- - -",
    "> - - -",
    "plain",
  ]) {
    const result = enter(text);
    assert(!result || (!result.value.endsWith("\n- ") && !result.value.endsWith("\n* ")));
  }
  assert.equal(enter("```\n```\n- real").value, "```\n```\n- real\n- ");
  assert.equal(enter("* - *").value, "* - *\n* ");
  assert.equal(edit("- code", 6, 6, "Enter", false, "```\n"), null);
  assert.equal(enter("    code").value, "    code\n    ");
  assert.equal(enter("```\n    code").value, "```\n    code\n    ");
});
test("Tab/Shift+Tab indent and outdent selected lines while preserving selection", () => {
  const input = "one\n  two\nthree";
  const result = edit(input, 0, 10, "Tab");
  assert.equal(result.value, "    one\n      two\nthree");
  assert.equal(result.start, 4);
  assert.equal(result.end, 18);
  const back = edit(result.value, result.start, result.end, "Tab", true);
  assert.equal(back.value, input);
  assert.equal(back.start, 0);
  assert.equal(back.end, 10);
  assert.equal(edit("first\nnext", 0, 6, "Tab").value, "    first\nnext");
  assert.equal(edit("    a\n\tb\n  c", 0, 13, "Tab", true).value, "a\nb\nc");
});
test("Tab indents whole list items; ordinary caret inserts spaces", () => {
  const result = edit("- hello", 4, 4, "Tab");
  assert.equal(result.value, "    - hello");
  assert.equal(result.start, 8);
  assert.equal(edit("    - hello", 8, 8, "Tab", true).value, "- hello");
  assert.equal(edit("hello", 2, 2, "Tab").value, "he    llo");
  assert.equal(edit("hello", 2, 2, "Tab", true).value, "hello");
});
const table = "| A | B |\n| :--- | ---: |\n| one | two |";
test("table navigation selects cells, skips separators and crosses rows", () => {
  let result = edit(table, table.indexOf("A"), table.indexOf("A") + 1, "Tab");
  assert.equal(table.slice(result.start, result.end), "B");
  assert.equal(result.value, table);
  result = edit(table, table.indexOf("B"), table.indexOf("B"), "Tab");
  assert.equal(table.slice(result.start, result.end), "one");
  result = edit(table, table.indexOf("one"), table.indexOf("one"), "Tab", true);
  assert.equal(table.slice(result.start, result.end), "B");
  result = edit(table, table.indexOf("B"), table.indexOf("B"), "Enter");
  assert.equal(table.slice(result.start, result.end), "two");
});
test("Tab at the last cell and Enter on the last row create a row before following prose", () => {
  const input = table + "\n\nAfter";
  for (const key of ["Tab", "Enter"]) {
    const result = edit(input, input.indexOf("two"), input.indexOf("two") + 3, key);
    assert.equal(result.value, table + "\n|   |   |\n\nAfter");
    assert.equal(result.start, result.end);
    assert.equal(result.start, table.length + (key === "Tab" ? 3 : 7));
  }
  const header = "| A | B |\n| --- | --- |\n";
  assert.equal(edit(header, 6, 6, "Tab").value, header.trimEnd() + "\n|   |   |\n");
});
test("tables handle escaped pipes, no outer pipes, Unicode and boundaries safely", () => {
  const input = "A | B\n--- | ---\n😀 a\\|b | c";
  const pos = input.indexOf("😀");
  const result = edit(input, pos, pos, "Tab");
  assert.equal(input.slice(result.start, result.end), "c");
  assert.equal(edit(table, 2, 2, "Tab", true).value, table);
  assert.equal(edit("plain | text", 3, 3, "Tab").value, "pla    in | text");
  const separator = table.indexOf(":---");
  assert.equal(edit(table, separator, separator, "Enter"), null);
  const code = "```\n" + table;
  assert.equal(edit(code, code.indexOf("two"), code.indexOf("two"), "Enter"), null);
});
test("keyboard adapter preserves direction and sends one input only for actual changes", () => {
  const textarea = {
    value: "- hello",
    selectionStart: 3,
    selectionEnd: 6,
    selectionDirection: "backward",
    setRangeText(insert, start, end) {
      this.value = this.value.slice(0, start) + insert + this.value.slice(end);
    },
    setSelectionRange(start, end, direction) {
      this.selectionStart = start;
      this.selectionEnd = end;
      this.selectionDirection = direction;
    },
    dispatchEvent(event) {
      assert.equal(event.type, "input");
      this.inputs = (this.inputs || 0) + 1;
    },
  };
  const event = {
    key: "Tab",
    preventDefault() {
      this.defaultPrevented = true;
    },
  };
  assert(handle(event, textarea));
  assert.equal(textarea.value, "    - hello");
  assert.equal(textarea.inputs, 1);
  assert.equal(textarea.selectionDirection, "backward");
  for (const props of [
    { ctrlKey: true },
    { metaKey: true },
    { altKey: true },
    { isComposing: true },
    { keyCode: 229 },
  ]) {
    assert.equal(handle({ ...event, defaultPrevented: false, ...props }, textarea), false);
  }
  assert.equal(
    handle({ ...event, defaultPrevented: false }, textarea, () => false),
    false,
  );
  textarea.readOnly = true;
  assert.equal(handle({ ...event, defaultPrevented: false }, textarea), false);
  assert.equal(textarea.inputs, 1);
});
