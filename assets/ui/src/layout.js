import { $, $$ } from "./dom.js";

// Pointer and keyboard resizing share the same bounds and persistence path.
export function createLayout(save) {
  let prefs = {
    filesWidth: 240,
    outlineWidth: 220,
    splitRatio: 0.5,
    showFiles: true,
    showOutline: true,
    focusMode: false,
  };
  let dragging = false;
  const workspace = $("#workspace");
  function apply() {
    const files = prefs.showFiles && !prefs.focusMode;
    const outline = prefs.showOutline && !prefs.focusMode;
    const budget = Math.max(0, workspace.clientWidth - 330);
    let left = files ? Math.min(prefs.filesWidth, Math.max(160, budget - (outline ? 165 : 0))) : 0;
    let right = outline ? Math.min(prefs.outlineWidth, Math.max(160, budget - left - 10)) : 0;
    // Preserve an editor even if a window temporarily becomes smaller than its native minimum.
    if (left + right + 10 > budget && outline) right = 0;
    if (left + right + 10 > budget && files) left = 0;
    const root = document.documentElement;
    root.style.setProperty("--files-size", `${left}px`);
    root.style.setProperty("--outline-size", `${right}px`);
    root.style.setProperty("--files-grip", left ? "5px" : "0px");
    root.style.setProperty("--outline-grip", right ? "5px" : "0px");
    root.style.setProperty("--source-ratio", `${prefs.splitRatio * 100}%`);
    $("#files-panel").hidden = !left;
    $("#files-resizer").hidden = !left;
    $("#outline-panel").hidden = !right;
    $("#outline-resizer").hidden = !right;
    $("#files-resizer").setAttribute("aria-valuenow", String(Math.round(left)));
    $("#outline-resizer").setAttribute("aria-valuenow", String(Math.round(right)));
    $("#split-resizer").setAttribute("aria-valuenow", String(Math.round(prefs.splitRatio * 100)));
    for (const id of ["files", "outline", "focus"]) {
      const key = id === "focus" ? "focusMode" : `show${id[0].toUpperCase()}${id.slice(1)}`;
      $(`#toggle-${id}`).setAttribute("aria-pressed", String(prefs[key]));
    }
    document.body.classList.toggle("focus-mode", prefs.focusMode);
  }
  function bind(id, key, min, max, direction, normal) {
    const handle = $(id);
    handle.setAttribute("aria-valuemin", String(min));
    handle.setAttribute("aria-valuemax", String(max));
    function update(value, persist) {
      prefs[key] = Math.max(min, Math.min(max, value));
      apply();
      if (persist) save({ [key]: prefs[key] });
    }
    handle.addEventListener("pointerdown", (event) => {
      if (event.button !== 0) return;
      event.preventDefault();
      const start = event.clientX,
        value = prefs[key];
      const width = $("#document").clientWidth;
      dragging = true;
      handle.setPointerCapture(event.pointerId);
      handle.classList.add("dragging");
      document.body.classList.add("resizing");
      const move = (e) =>
        update(
          value + ((e.clientX - start) * direction) / (key === "splitRatio" ? width : 1),
          false,
        );
      const stop = () => {
        dragging = false;
        handle.classList.remove("dragging");
        document.body.classList.remove("resizing");
        handle.removeEventListener("pointermove", move);
        handle.removeEventListener("pointerup", stop);
        handle.removeEventListener("pointercancel", stop);
        save({ [key]: prefs[key] });
      };
      handle.addEventListener("pointermove", move);
      handle.addEventListener("pointerup", stop);
      handle.addEventListener("pointercancel", stop);
    });
    handle.addEventListener("keydown", (event) => {
      const step = key === "splitRatio" ? 0.025 : 16;
      const delta = event.key === "ArrowLeft" ? -step : event.key === "ArrowRight" ? step : 0;
      if (!delta && event.key !== "Home" && event.key !== "End") return;
      event.preventDefault();
      update(
        event.key === "Home" ? min : event.key === "End" ? max : prefs[key] + delta * direction,
        true,
      );
    });
    handle.addEventListener("dblclick", () => update(normal, true));
  }
  bind("#files-resizer", "filesWidth", 160, 560, 1, 240);
  bind("#outline-resizer", "outlineWidth", 160, 480, -1, 220);
  bind("#split-resizer", "splitRatio", 0.2, 0.8, 1, 0.5);
  new ResizeObserver(apply).observe(workspace);
  return {
    accept(incoming) {
      if (!dragging) prefs = { ...incoming };
      apply();
    },
    reset() {
      prefs.filesWidth = 240;
      prefs.outlineWidth = 220;
      prefs.splitRatio = 0.5;
      apply();
      save({ filesWidth: 240, outlineWidth: 220, splitRatio: 0.5 });
    },
  };
}
