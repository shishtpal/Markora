import { $, $$ } from "./dom.js";

export function createConflictDialog(send) {
  const dialog = $("#conflict-dialog");
  let token = null;
  $$("[data-conflict]").forEach((button) => {
    button.onclick = () =>
      send({ type: "resolveConflict", choice: button.dataset.conflict, token });
  });
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    send({ type: "resolveConflict", choice: "cancel", token });
  });
  return {
    accept(snapshot) {
      const conflict = snapshot.conflict;
      token = conflict?.token ?? null;
      if (!conflict) {
        if (dialog.open) dialog.close();
        return;
      }
      $("#conflict-path").textContent = conflict.path;
      $("#conflict-description").textContent = conflict.deleted
        ? "This file was deleted outside Markora. Keep your draft to recreate it, or save a copy elsewhere."
        : "This file changed outside Markora. Reload replaces your draft with the disk version. Keep Mine preserves your draft for saving.";
      $('[data-conflict="reload"]').disabled = conflict.deleted;
      if (!dialog.open) {
        $$("dialog[open]").forEach((other) => other.close());
        dialog.showModal();
      }
    },
  };
}
