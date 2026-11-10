import { $, resolveMedia } from "./dom.js";
import { createLiveEditor } from "./live-editor.js";

export function createEditor(onChange) {
  let doc = {
    id: 0,
    markdown: "",
    revision: 0,
    viewMode: "live",
    rendered: { html: "", blocks: [], outline: [] },
    renderedMarkdown: "",
  };
  const source = $("#source-editor");
  const histories = new Map();
  let currentHistory;
  let lastPreview = null;
  const live = createLiveEditor($("#live-document"), () => doc, change);

  function change(markdown, record = true) {
    if (markdown === doc.markdown) return;
    doc.markdown = markdown;
    doc.revision++;
    if (record) {
      currentHistory.undo.push(markdown);
      if (currentHistory.undo.length > 100) currentHistory.undo.shift();
      currentHistory.redo = [];
    }
    if (source.value !== markdown) source.value = markdown;
    onChange();
  }
  source.addEventListener("input", () => change(source.value));
  source.addEventListener("keydown", (event) => {
    if (event.key !== "Tab") return;
    event.preventDefault();
    source.setRangeText("    ", source.selectionStart, source.selectionEnd, "end");
    source.dispatchEvent(new Event("input"));
  });
  $("#append-block").addEventListener("click", () => live.append());

  function accept(incoming) {
    const switched = incoming.id !== doc.id;
    if (!switched && incoming.revision < doc.revision) return;
    if (switched) {
      live.reset();
      lastPreview = null;
      if (!histories.has(incoming.id))
        histories.set(incoming.id, { undo: [incoming.markdown], redo: [] });
      currentHistory = histories.get(incoming.id);
    }
    const modeChanged = incoming.viewMode !== doc.viewMode;
    if (modeChanged) live.finish();
    doc = { ...incoming, renderedMarkdown: incoming.markdown };
    if (source.value !== doc.markdown) source.value = doc.markdown;
    $("#document").dataset.mode = doc.viewMode;
    $("#live-pane").hidden = doc.viewMode !== "live";
    $("#source-pane").hidden = doc.viewMode === "live";
    $("#split-resizer").hidden = doc.viewMode !== "split";
    if (lastPreview !== doc.rendered.html || switched) {
      $("#preview-document").innerHTML = doc.rendered.html;
      resolveMedia($("#preview-document"), doc.id);
      lastPreview = doc.rendered.html;
    }
    live.render();
    if (switched && doc.viewMode === 'live' && !doc.markdown) live.append();
    if (switched) {
      source.scrollTop = 0;
      $("#live-pane").scrollTop = 0;
    }
    if ((modeChanged || switched) && doc.viewMode !== "live") source.focus();
  }
  function editingSurface() {
    return doc.viewMode === "live" ? live.textarea() || live.focus() : source;
  }
  function replaceSelection(prefix, suffix = "", placeholder = "text") {
    const textarea = editingSurface();
    if (!textarea) return;
    const start = textarea.selectionStart,
      end = textarea.selectionEnd;
    const text = textarea.value.slice(start, end) || placeholder;
    textarea.setRangeText(prefix + text + suffix, start, end, "end");
    textarea.setSelectionRange(start + prefix.length, start + prefix.length + text.length);
    textarea.dispatchEvent(new Event("input"));
    textarea.focus();
  }
  function format(command, url) {
    const wraps = {
      bold: ["**", "**"],
      italic: ["*", "*"],
      code: ["`", "`"],
      strike: ["~~", "~~"],
      link: ["[", `](${url})`],
    };
    if (wraps[command]) {
      replaceSelection(...wraps[command]);
      return;
    }
    const inserts = {
      "code-block": ["\n```\n", "\n```\n", "code"],
      table: ["\n\n", "\n", "| Column A | Column B |\n| --- | --- |\n| one | two |"],
    };
    if (inserts[command]) {
      replaceSelection(...inserts[command]);
      return;
    }
    const prefixes = {
      "heading-1": "# ",
      "heading-2": "## ",
      "heading-3": "### ",
      list: "- ",
      "ordered-list": "1. ",
      quote: "> ",
    };
    const textarea = editingSurface();
    if (!textarea || !prefixes[command]) return;
    const start = textarea.value.lastIndexOf("\n", textarea.selectionStart - 1) + 1;
    const end = textarea.value.indexOf("\n", textarea.selectionEnd);
    const finish = end < 0 ? textarea.value.length : end;
    const lines = textarea.value
      .slice(start, finish)
      .split("\n")
      .map((line) => prefixes[command] + line.replace(/^(#{1,6} |[-*+] |> |\d+\. )/, ""));
    textarea.setRangeText(lines.join("\n"), start, finish, "end");
    textarea.dispatchEvent(new Event("input"));
    textarea.focus();
  }
  function history(direction) {
    if (!currentHistory) return;
    const { undo, redo } = currentHistory;
    let text;
    if (direction === "undo" && undo.length > 1) {
      redo.push(undo.pop());
      text = undo.at(-1);
    }
    if (direction === "redo" && redo.length) {
      text = redo.pop();
      undo.push(text);
    }
    if (text === undefined) return;
    live.reset();
    change(text, false);
  }
  return {
    accept,
    format,
    history,
    draft: () =>
      doc.id ? { docId: doc.id, markdown: doc.markdown, revision: doc.revision } : null,
    document: () => doc,
    focus() {
      if (doc.viewMode === "live") live.focus();
      else source.focus();
    },
    jump(offset, length = 0) {
      if (doc.viewMode === "live") live.jump(offset, length);
      else {
        source.focus();
        source.setSelectionRange(offset, offset + length);
        const line = doc.markdown.slice(0, offset).split("\n").length - 1;
        source.scrollTop = Math.max(
          0,
          line * parseFloat(getComputedStyle(source).lineHeight) - source.clientHeight / 2,
        );
      }
    },
  };
}
