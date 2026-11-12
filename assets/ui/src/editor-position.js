import { $ } from "./dom.js";

// Browser-only tab state. Save before replacing shared DOM; restore after layout.
export function createEditorPosition(getDocument, source, live, editMode, isReading) {
  const states = new Map();
  const preview = $("#preview-pane");
  let pending = null;
  let frame = null;
  function save() {
    const doc = getDocument();
    if (!doc.id || doc.kind === "pdf" || pending?.id === doc.id || isReading()) return;
    const state = states.get(doc.id) || {};
    if (doc.viewMode === "live") {
      state.live = live.capturePosition();
      state.enabled = editMode.enabled();
    } else {
      state.source = {
        selection: [source.selectionStart, source.selectionEnd, source.selectionDirection],
        top: source.scrollTop,
        left: source.scrollLeft,
      };
      if (doc.viewMode === "split") state.preview = [preview.scrollTop, preview.scrollLeft];
    }
    states.set(doc.id, state);
  }
  function apply() {
    if (!pending || frame !== null) return;
    const restore = pending;
    frame = requestAnimationFrame(() => {
      frame = null;
      const doc = getDocument();
      if (pending !== restore || restore.id !== doc.id || restore.mode !== doc.viewMode) return;
      if (isReading() || doc.kind === "pdf") {
        pending = null;
        return;
      }
      const state = states.get(doc.id) || {};
      if (doc.viewMode === "live") {
        if (
          !live.restorePosition(
            state.live || { anchor: 0, offset: 0, top: 0, left: 0 },
            editMode.canEdit(),
          )
        )
          return;
        // Deferred batches resolve their intrinsic heights on the next layout.
        if (!restore.settled) {
          restore.settled = true;
          apply();
          return;
        }
      } else {
        source.focus({ preventScroll: true });
        source.setSelectionRange(...(state.source?.selection || [0, 0, "none"]));
        source.scrollTop = state.source?.top || 0;
        source.scrollLeft = state.source?.left || 0;
        if (doc.viewMode === "split") {
          preview.scrollTop = state.preview?.[0] || 0;
          preview.scrollLeft = state.preview?.[1] || 0;
        }
      }
      pending = null;
    });
  }
  return {
    save,
    apply,
    restore(switched) {
      const doc = getDocument();
      if (switched && doc.viewMode === "live") editMode.restore(states.get(doc.id)?.enabled);
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      pending = { id: doc.id, mode: doc.viewMode };
      apply();
    },
    retain(tabs, histories) {
      const ids = new Set(tabs.map((tab) => tab.id));
      for (const map of [states, histories]) {
        for (const id of map.keys()) if (!ids.has(id)) map.delete(id);
      }
    },
  };
}
