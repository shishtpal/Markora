import { $ } from "./dom.js";

// Recreate the editing block from source offsets, without scrolling the caret into view.
export function createLivePosition(root, getDocument, getActive, begin, ensureRenderedThrough) {
  const pane = $("#live-pane");
  function selection() {
    const active = getActive();
    return active
      ? {
          start: active.before.length + active.textarea.selectionStart,
          end: active.before.length + active.textarea.selectionEnd,
          direction: active.textarea.selectionDirection,
        }
      : null;
  }
  return {
    selection,
    capture() {
      const rect = pane.getBoundingClientRect();
      // Hit testing avoids laying out offscreen content-visibility batches.
      let node = null;
      for (let y = rect.top + 1; y < rect.bottom && !node; y += 16) {
        node = document.elementFromPoint(rect.left + rect.width / 2, y)?.closest("[data-block]");
        if (node && !root.contains(node)) node = null;
      }
      const textarea = getActive()?.textarea;
      return {
        selection: selection(),
        empty: getActive()?.index === -1 && !textarea.value,
        anchor: node ? Number(node.dataset.block) : null,
        offset: node ? node.getBoundingClientRect().top - rect.top : 0,
        top: pane.scrollTop,
        left: pane.scrollLeft,
        textareaScroll: textarea ? [textarea.scrollTop, textarea.scrollLeft] : [0, 0],
      };
    },
    restore(state, editable, append) {
      const doc = getDocument();
      if (doc.renderedMarkdown !== doc.markdown) return false;
      const blocks = doc.rendered.blocks;
      const anchor = state.anchor === null ? -1 : Math.min(state.anchor, blocks.length - 1);
      if (anchor >= 0) ensureRenderedThrough(anchor);
      if (editable && state.selection) {
        const { start, end, direction } = state.selection;
        const index = blocks.findIndex((block) => block.end > start);
        const target = index < 0 ? blocks.length - 1 : index;
        const textarea = state.empty || target < 0 ? append() : begin(target);
        if (!textarea) return false;
        const offset = getActive().before.length;
        textarea.setSelectionRange(
          Math.max(0, start - offset),
          Math.max(0, end - offset),
          direction,
        );
        textarea.scrollTop = state.textareaScroll[0];
        textarea.scrollLeft = state.textareaScroll[1];
        textarea.focus({ preventScroll: true });
      }
      const node = root.querySelector(`[data-block="${anchor}"]`);
      if (node) {
        node.scrollIntoView({ block: "start" });
        pane.scrollTop +=
          node.getBoundingClientRect().top - pane.getBoundingClientRect().top - state.offset;
      } else pane.scrollTop = state.top;
      pane.scrollLeft = state.left;
      return true;
    },
  };
}
