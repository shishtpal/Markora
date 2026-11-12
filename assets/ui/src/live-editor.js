import { connectLiveTasks } from "./live-tasks.js";
import { element, resolveMedia } from "./dom.js";

const INITIAL_BATCH = 60;
const FRAME_BATCH = 150;
const MAX_FRAME_BLOCKS = 1200;

// Edit Markdown source for one rendered block. Never reverse-engineer Markdown from HTML.
export function createLiveEditor(root, getDocument, change, flushEdit = () => {}, canEdit = () => true) {
  let active = null;
  let renderedMarkdown = null;
  let renderedBlocksRef = null;
  let nextBlock = null;
  let rafId = null;
  let renderedCount = 0;

  function cancelQueue() {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
  }
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
  function buildBlockNode(block, index, docId) {
    const node = element("div", "live-block");
    node.innerHTML = block.html;
    if (
      block.html.includes("<img") ||
      block.html.includes("<audio") ||
      block.html.includes("<video") ||
      block.html.includes("<source")
    ) {
      resolveMedia(node, docId);
    }
    if (block.html.includes('type="checkbox"')) {
      node.querySelectorAll('li input[type="checkbox"]').forEach((box) => {
        box.disabled = !canEdit();
      });
    }
    node.tabIndex = 0;
    node.dataset.block = String(index);
    node.setAttribute("aria-label", `Markdown block ${index + 1}`);
    return node;
  }
  function appendRange(blocks, docId, start, end) {
    const limit = Math.min(end, blocks.length);
    if (start >= limit) return;
    const fragment = document.createDocumentFragment();
    for (let first = start; first < limit; first += FRAME_BATCH) {
      const batch = element("div", first ? "live-batch deferred" : "live-batch");
      const batchEnd = Math.min(first + FRAME_BATCH, limit);
      batch.style.setProperty("--batch-height", `${(batchEnd - first) * 64}px`);
      for (let i = first; i < batchEnd; i++) batch.append(buildBlockNode(blocks[i], i, docId));
      fragment.append(batch);
    }
    root.append(fragment);
    renderedCount = limit;
  }
  function ensureRenderedThrough(targetIndex) {
    const doc = getDocument();
    const blocks = doc.rendered.blocks;
    if (renderedCount > targetIndex || renderedCount >= blocks.length) return;
    cancelQueue();
    appendRange(blocks, doc.id, renderedCount, targetIndex + 20);
    scheduleRemaining(blocks, doc.id);
  }
  function scheduleRemaining(blocks, docId) {
    if (rafId !== null || active || renderedCount >= blocks.length) return;
    rafId = requestAnimationFrame(() => {
      rafId = null;
      const doc = getDocument();
      if (doc.rendered.blocks !== blocks || doc.viewMode !== "live" || active) return;
      const size = Math.max(FRAME_BATCH, Math.min(MAX_FRAME_BLOCKS, renderedCount));
      appendRange(blocks, docId, renderedCount, renderedCount + size);
      scheduleRemaining(blocks, docId);
    });
  }
  function render() {
    const doc = getDocument();
    if (active || doc.viewMode !== "live" || doc.renderedMarkdown !== doc.markdown) return;
    if (renderedMarkdown === doc.markdown && renderedBlocksRef === doc.rendered.blocks) {
      scheduleRemaining(doc.rendered.blocks, doc.id);
      return;
    }
    cancelQueue();
    renderedMarkdown = doc.markdown;
    renderedBlocksRef = doc.rendered.blocks;
    renderedCount = 0;
    const blocks = doc.rendered.blocks;
    root.replaceChildren();
    appendRange(blocks, doc.id, 0, INITIAL_BATCH);
    if (nextBlock !== null) {
      const index = Math.min(nextBlock, blocks.length - 1);
      nextBlock = null;
      if (index >= 0) begin(index);
    }
    scheduleRemaining(blocks, doc.id);
  }
  function begin(index) {
    if (!canEdit()) return null;
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
    ensureRenderedThrough(index);
    const block = doc.rendered.blocks[index];
    const node = root.querySelector(`[data-block="${index}"]`);
    if (!block || !node) return null;
    cancelQueue();
    const textarea = element("textarea", "block-editor");
    textarea.setAttribute("aria-label", "Edit Markdown block");
    textarea.spellcheck = false;
    textarea.value = doc.markdown.slice(block.start, block.end);
    active = {
      index,
      node,
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
    const editing = active;
    active = null;
    if (editing) {
      const doc = getDocument();
      if (renderedMarkdown === doc.markdown && renderedBlocksRef === doc.rendered.blocks) {
        if (editing.index >= 0)
          editing.node.replaceWith(
            buildBlockNode(doc.rendered.blocks[editing.index], editing.index, doc.id),
          );
        else editing.node.remove();
      } else renderedMarkdown = null;
      flushEdit();
    }
    render();
  }
  function append(offset = getDocument().markdown.length) {
    if (!canEdit()) return null;
    finish();
    const doc = getDocument();
    ensureRenderedThrough(doc.rendered.blocks.length);
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
      node,
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
  connectLiveTasks(root, getDocument, change, canEdit);
  root.addEventListener("click", (event) => {
    if (event.target.closest("a, input, textarea, button, audio, video")) return;
    const node = event.target.closest("[data-block]");
    if (node) begin(Number(node.dataset.block));
  });
  root.addEventListener("keydown", (event) => {
    if (!canEdit()) return;
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
    selection() {
      return active
        ? {
            start: active.before.length + active.textarea.selectionStart,
            end: active.before.length + active.textarea.selectionEnd,
          }
        : null;
    },
    render,
    finish,
    append,
    reset() {
      cancelQueue();
      active = null;
      renderedMarkdown = null;
      renderedBlocksRef = null;
      renderedCount = 0;
      nextBlock = null;
      root.replaceChildren();
    },
    textarea() {
      return active?.textarea || null;
    },
    focus() {
      if (!canEdit()) { root.focus(); return null; }
      return active?.textarea || begin(0) || append();
    },
    jump(offset, length = 0) {
      finish();
      const index = getDocument().rendered.blocks.findIndex((b) => b.end > offset);
      const target = Math.max(0, index);
      if (!canEdit()) {
        ensureRenderedThrough(target);
        const node = root.querySelector(`[data-block="${target}"]`);
        node?.focus({ preventScroll: true });
        node?.scrollIntoView({ block: "center" });
        return;
      }
      const textarea = begin(target);
      if (textarea) {
        const start = getDocument().rendered.blocks[target]?.start || 0;
        textarea.setSelectionRange(offset - start, offset - start + length);
        textarea.scrollIntoView({ block: "center" });
      }
    },
  };
}
