import { $, $$ } from "./dom.js";

// Live editing is a deliberate, document-local choice. Source/Split are editors.
export function createEditMode(getDocument, isReading, live, flush) {
  let enabled = false;
  const canEdit = () =>
    !isReading() && getDocument().kind !== "pdf" && (getDocument().viewMode !== "live" || enabled);
  function paint() {
    const doc = getDocument(),
      liveMode = doc.kind !== "pdf" && doc.viewMode === "live";
    $("#toggle-edit").hidden = !liveMode;
    $("#toggle-edit").setAttribute("aria-pressed", String(enabled));
    $("#toggle-edit-label").textContent = enabled ? "Done editing" : "Edit mode";
    $("#live-access").hidden = !liveMode;
    $("#live-access").textContent = enabled ? "Editing" : "Read-only";
    $("#append-block").hidden = !liveMode || !canEdit();
    $("#live-document").classList.toggle("read-only", !canEdit());
    for (const box of $$('#live-document li input[type="checkbox"]')) box.disabled = !canEdit();
    for (const button of $$("#formatbar button")) button.disabled = !canEdit();
  }
  return {
    canEdit,
    enabled: () => enabled,
    restore(value) { enabled = Boolean(value); paint(); },
    reset() {
      enabled = false;
    },
    paint,
    toggle() {
      if (isReading() || getDocument().kind === "pdf" || getDocument().viewMode !== "live") return;
      if (enabled) {
        live.finish();
        flush();
      }
      enabled = !enabled;
      paint();
      if (enabled && !getDocument().markdown) live.append();
    },
  };
}
