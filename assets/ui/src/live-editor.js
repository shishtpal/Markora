import { element, resolveMedia } from "./dom.js";

// Edit Markdown source for one rendered block. Never reverse-engineer Markdown from HTML.
export function createLiveEditor(root, getDocument, change) {
  let active = null;
  let renderedMarkdown = null;
  let nextBlock = null;

  function resize(textarea) {
    textarea.style.height = "0px";
    textarea.style.height = `${Math.max(52, textarea.scrollHeight)}px`;
  }
  function hintBar(label) {
    const bar = element("div", "block-hint");
    const keys = element("span", "block-hint-keys");
    keys.innerHTML = label;
    const done = element("button", "block-done", "Done");
    done.type = "button";
    done.onmousedown = (e) => e.preventDefault();
    done.onclick = (e) => {
      e.stopPropagation();
      finish();
    };
    bar.append(keys, done);
    return bar;
  }
  function render() {
    const doc = getDocument();
    if (active || doc.renderedMarkdown !== doc.markdown) return;
    renderedMarkdown = doc.markdown;
    const nodes = doc.rendered.blocks.map((block, index) => {
      const node = element("div", "live-block");
      node.innerHTML = block.html;
      resolveMedia(node, doc.id);
      node.querySelectorAll('li input[type="checkbox"]').forEach((box) => {
        box.disabled = false;
      });
      node.tabIndex = 0;
      node.dataset.block = String(index);
      node.setAttribute("aria-label", `Edit block ${index + 1}`);
      return node;
    });
    root.replaceChildren(...nodes);
    if (nextBlock !== null) {
      const index = Math.min(nextBlock, nodes.length - 1);
      nextBlock = null;
      if (index >= 0) begin(index);
    }
  }
  function begin(index) {
    if (active?.index === index) {
      active.textarea.focus();
      return active.textarea;
    }
    finish();
    const doc = getDocument();
    if (renderedMarkdown !== doc.markdown) {
      nextBlock = index;
      return null;
    }
    const block = doc.rendered.blocks[index];
    const node = root.querySelector(`[data-block="${index}"]`);
    if (!block || !node) return null;
    const textarea = element("textarea", "block-editor");
    textarea.setAttribute("aria-label", "Edit Markdown block");
    textarea.spellcheck = false;
    textarea.value = doc.markdown.slice(block.start, block.end);
    active = {
      index,
      textarea,
      before: doc.markdown.slice(0, block.start),
      after: doc.markdown.slice(block.end),
    };
    node.classList.add("editing");
    node.removeAttribute("tabindex");
    node.replaceChildren(
      textarea,
      hintBar("<kbd>Esc</kbd> render · <kbd>Ctrl+Enter</kbd> new paragraph"),
    );
    textarea.addEventListener("input", () => {
      resize(textarea);
      change(active.before + textarea.value + active.after);
    });
    textarea.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        finish();
      }
      if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
        event.preventDefault();
        const offset = active.before.length + textarea.value.length;
        append(offset);
      }
      if (event.key === "Tab") {
        event.preventDefault();
        textarea.setRangeText("    ", textarea.selectionStart, textarea.selectionEnd, "end");
        textarea.dispatchEvent(new Event("input"));
      }
    });
    resize(textarea);
    textarea.focus();
    return textarea;
  }
  function finish() {
    active = null;
    render();
  }
  function append(offset = getDocument().markdown.length) {
    finish();
    const doc = getDocument();
    const before = doc.markdown.slice(0, offset).replace(/\n*$/, "");
    const after = doc.markdown.slice(offset).replace(/^\n*/, "");
    // A real empty Markdown editing block, without inserting placeholder text into the file.
    const textarea = element("textarea", "block-editor");
    textarea.setAttribute("aria-label", "Edit Markdown block");
    const node = element("div", "live-block editing");
    node.append(textarea, hintBar("New paragraph · <kbd>Esc</kbd> render"));
    root.append(node);
    active = {
      index: -1,
      textarea,
      before: before ? before + "\n\n" : "",
      after: after ? "\n\n" + after : "",
    };
    textarea.addEventListener("input", () => {
      resize(textarea);
      change(active.before + textarea.value + active.after);
    });
    textarea.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        finish();
      }
    });
    resize(textarea);
    textarea.focus();
    node.scrollIntoView({ block: "nearest" });
    return textarea;
  }
  root.addEventListener("change", (event) => {
    const box = event.target.closest('li input[type="checkbox"]');
    const node = box?.closest("[data-block]");
    if (!box || !node) return;
    const doc = getDocument();
    const block = doc.rendered.blocks[Number(node.dataset.block)];
    if (!block) return;
    const targetIndex = [...node.querySelectorAll('li input[type="checkbox"]')].indexOf(box);
    let seen = -1;
    let fence = false;
    const slice = doc.markdown.slice(block.start, block.end);
    const updated = slice
      .split("\n")
      .map((line) => {
        if (/^[ \t]*(?:```|~~~)/.test(line)) fence = !fence;
        if (fence) return line;
        return line.replace(/^([ \t]*(?:[-*+]|\d+[.)])\s+\[)([ xX])(\])/, (_, pre, _mark, post) => {
          seen += 1;
          return seen === targetIndex ? `${pre}${box.checked ? "x" : " "}${post}` : _;
        });
      })
      .join("\n");
    if (updated !== slice) change(doc.markdown.slice(0, block.start) + updated + doc.markdown.slice(block.end));
  });
  root.addEventListener("click", (event) => {
    if (event.target.closest("a, input, textarea, button, audio, video")) return;
    const node = event.target.closest("[data-block]");
    if (node) begin(Number(node.dataset.block));
  });
  root.addEventListener("keydown", (event) => {
    if (event.target.tagName === "TEXTAREA") return;
    if (event.key === "Enter" || event.key === " ") {
      const node = event.target.closest("[data-block]");
      if (node) {
        event.preventDefault();
        begin(Number(node.dataset.block));
      }
    }
  });
  return {
    render,
    finish,
    append,
    reset() {
      active = null;
      renderedMarkdown = null;
      nextBlock = null;
      root.replaceChildren();
    },
    textarea() {
      return active?.textarea || null;
    },
    focus() {
      return active?.textarea || begin(0) || append();
    },
    jump(offset, length = 0) {
      finish();
      const index = getDocument().rendered.blocks.findIndex((b) => b.end > offset);
      const textarea = begin(Math.max(0, index));
      if (textarea) {
        const start = getDocument().rendered.blocks[Math.max(0, index)]?.start || 0;
        textarea.setSelectionRange(offset - start, offset - start + length);
        textarea.scrollIntoView({ block: "center" });
      }
    },
  };
}
