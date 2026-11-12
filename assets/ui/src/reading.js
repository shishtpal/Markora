import { $, $$ } from "./dom.js";

// The native host owns the mode; this module preserves transient editing UI.
export function createReading(editor) {
  let active = false,
    saved = null,
    transition = 0,
    previewDoc = null;
  const preview = $("#preview-pane");
  const positions = new Map();
  const hidden = [
    "#menubar",
    "#tabbar",
    "#viewbar",
    "#statusbar",
    "#files-panel",
    "#outline-panel",
    "#files-resizer",
    "#outline-resizer",
    "#source-pane",
    "#live-pane",
    "#split-resizer",
  ].map($);

  function capture() {
    const focus = document.activeElement;
    return {
      id: editor.document().id,
      focus,
      selection:
        focus instanceof HTMLTextAreaElement
          ? [focus.selectionStart, focus.selectionEnd, focus.selectionDirection]
          : null,
      scroll: [$("#source-editor"), $("#live-pane"), preview, $(".block-editor")]
        .filter(Boolean)
        .map((node) => [node, node.scrollTop, node.scrollLeft]),
    };
  }
  function focus() {
    (editor.document().kind === "pdf" ? $("#pdf-frame") : preview).focus({ preventScroll: true });
  }
  function jump(offset) {
    const blocks = $$("#preview-document [data-source-end]");
    const block = blocks.find((b) => Number(b.dataset.sourceEnd) > offset) || blocks.at(-1);
    $$(".reading-match").forEach((b) => b.classList.remove("reading-match"));
    if (block) {
      block.classList.add("reading-match");
      block.scrollIntoView({ block: "center" });
    }
    focus();
  }
  function before(next) {
    if (active && previewDoc !== null) positions.set(previewDoc, [preview.scrollTop, preview.scrollLeft]);
    next = Boolean(next);
    if (next === active) return;
    if (next) editor.capturePosition();
    transition++;
    if (next) saved = capture();
    active = next;
    document.body.classList.toggle("reading-mode", active);
    hidden.forEach((node) => {
      node.inert = active;
    });
    $("#exit-reading").hidden = !active;
    $("#find-toggle").disabled = active;
    if (active) {
      $("#replace-row").hidden = true;
      $("#find-toggle").setAttribute("aria-expanded", "false");
    }
    $$("[data-command='toggle-reading']").forEach((button) => {
      button.setAttribute("aria-pressed", String(active));
    });

    if (!active) {
      editor.resumeEditing();
      const state = saved,
        token = transition;
      saved = null;
      previewDoc = null;
      $$(".reading-match").forEach((b) => b.classList.remove("reading-match"));
      requestAnimationFrame(() => {
        if (active || token !== transition) return;
        if (state?.id === editor.document().id) {
          if (state.focus?.isConnected && state.focus.getClientRects().length) {
            state.focus.focus({ preventScroll: true });
            if (state.selection) state.focus.setSelectionRange(...state.selection);
          } else editor.restorePosition();
          for (const [node, top, left] of state.scroll) {
            if (node.isConnected) {
              node.scrollTop = top;
              node.scrollLeft = left;
            }
          }
        } else editor.restorePosition();
      });
    }
  }
  function accept(tabs) {
    const ids = new Set(tabs.map((tab) => tab.id));
    for (const id of positions.keys()) if (!ids.has(id)) positions.delete(id);
    if (!active) return;
    const doc = editor.document();
    if (doc.kind !== "pdf") editor.preparePrint();
    if (previewDoc !== doc.id) {
      previewDoc = doc.id;
      preview.scrollTop = positions.get(doc.id)?.[0] || 0;
      preview.scrollLeft = positions.get(doc.id)?.[1] || 0;
      if (!$("dialog[open]")) focus();
    }
  }
  // Task lists remain selectable and copyable, but cannot change in reading mode.
  $("#preview-document").addEventListener("click", (event) => {
    if (active && event.target.closest("input")) event.preventDefault();
  });
  return { active: () => active, before, accept, focus, jump };
}
