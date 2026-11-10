import { $, resolveMedia } from "./dom.js";
import { createLiveEditor } from "./live-editor.js";

export function createEditor(onChange, onFlush = () => {}) {
  let doc = {
    id: 0,
    markdown: "",
    revision: 0,
    viewMode: "live",
    rendered: { blocks: [], outline: [] },
    renderedMarkdown: "",
    previewHtml: null,
  };
  const source = $("#source-editor");
  const histories = new Map();
  let currentHistory;
  let lastPreview = null;
  let lastPreviewDocId = 0;
  const live = createLiveEditor($("#live-document"), () => doc, change, onFlush);

  function change(markdown, record = true, immediate = false) {
    if (markdown === doc.markdown) return;
    doc.markdown = markdown;
    doc.revision++;
    if (record) {
      currentHistory.undo.push(markdown);
      if (currentHistory.undo.length > 100) currentHistory.undo.shift();
      currentHistory.redo = [];
    }
    if (doc.viewMode !== "live" && source.value !== markdown) source.value = markdown;
    const delay = immediate || markdown.length <= 100_000 ? 0 : 160;
    onChange(delay);
  }
  function ensurePreview() {
    if (doc.previewHtml === null) {
      const batches = [];
      const blocks = doc.rendered.blocks;
      for (let start = 0; start < blocks.length; start += 150) {
        const slice = blocks.slice(start, start + 150);
        batches.push(`<div class="preview-batch${start ? " deferred" : ""}" style="--batch-height:${slice.length * 64}px">${slice.map((b) => b.html).join("")}</div>`);
      }
      doc.previewHtml = batches.join("");
    }
    if (lastPreview !== doc.previewHtml || lastPreviewDocId !== doc.id) {
      $("#preview-document").innerHTML = doc.previewHtml;
      resolveMedia($("#preview-document"), doc.id);
      lastPreview = doc.previewHtml;
      lastPreviewDocId = doc.id;
    }
  }
  window.addEventListener("beforeprint", ensurePreview);
  window.matchMedia("print").addEventListener("change", (event) => {
    if (event.matches) ensurePreview();
  });

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
    const markdown = incoming.markdown ?? doc.markdown;
    const rendered = incoming.rendered ?? doc.rendered;
    const hasNewRendered = Boolean(incoming.rendered);
    if (switched) {
      live.reset();
      lastPreview = null;
      lastPreviewDocId = 0;
      if (!histories.has(incoming.id))
        histories.set(incoming.id, { undo: [markdown], redo: [] });
      currentHistory = histories.get(incoming.id);
    }
    const modeChanged = incoming.viewMode !== doc.viewMode;
    if (modeChanged) live.reset();
    const canLiveRender =
      hasNewRendered && (incoming.viewMode !== "source" || rendered.blocks.length > 0 || !markdown);
    doc = {
      id: incoming.id,
      markdown,
      revision: incoming.revision,
      viewMode: incoming.viewMode,
      rendered,
      renderedMarkdown: switched
        ? (incoming.viewMode !== "source" || rendered.blocks.length > 0 || !markdown ? markdown : "")
        : canLiveRender
          ? markdown
          : doc.renderedMarkdown,
      previewHtml: hasNewRendered || switched ? null : doc.previewHtml,
    };
    if (doc.viewMode !== "live" && source.value !== doc.markdown) source.value = doc.markdown;
    else if (switched && source.value !== doc.markdown && doc.markdown.length <= 100_000) {
      source.value = doc.markdown;
    }
    $("#document").dataset.mode = doc.viewMode;
    $("#live-pane").hidden = doc.viewMode !== "live";
    $("#source-pane").hidden = doc.viewMode === "live";
    $("#split-resizer").hidden = doc.viewMode !== "split";
    if (doc.viewMode === "split") ensurePreview();
    if (doc.viewMode === "live") live.render();
    if (switched && doc.viewMode === "live" && !doc.markdown) live.append();
    if (switched) {
      source.scrollTop = 0;
      $("#live-pane").scrollTop = 0;
    }
    if ((modeChanged || switched) && doc.viewMode !== "live") {
      if (source.value !== doc.markdown) source.value = doc.markdown;
      source.focus();
    }
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
    change(text, false, true);
  }
  return {
    accept,
    format,
    history,
    preparePrint: ensurePreview,
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
        if (source.value !== doc.markdown) source.value = doc.markdown;
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
