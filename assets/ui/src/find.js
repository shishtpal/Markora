import { $ } from "./dom.js";

export function createFind(editor) {
  const input = $("#find-input");
  let index = -1;
  function matches() {
    if (!input.value) return [];
    const haystack = editor.document().markdown.toLowerCase();
    const needle = input.value.toLowerCase();
    const hits = [];
    for (
      let at = haystack.indexOf(needle);
      at >= 0;
      at = haystack.indexOf(needle, at + needle.length)
    )
      hits.push(at);
    return hits;
  }
  function update(jump = false, direction = 1) {
    const hits = matches();
    if (jump && hits.length) index = index < 0 ? (direction > 0 ? 0 : hits.length - 1) : (index + direction + hits.length) % hits.length;
    if (index >= hits.length) index = hits.length - 1;
    $("#find-count").textContent = input.value
      ? `${index >= 0 ? index + 1 : 0} / ${hits.length}`
      : "";
    $("#find-count").classList.toggle("no-matches", Boolean(input.value) && hits.length === 0);
    if (jump && hits.length) editor.jump(hits[index], input.value.length);
  }
  input.addEventListener("input", () => {
    index = -1;
    update();
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      update(true, event.shiftKey ? -1 : 1);
    }
    if (event.key === "Escape") close();
  });
  function open() {
    $("#findbar").hidden = false;
    input.focus();
    input.select();
    update();
  }
  function close() {
    $("#findbar").hidden = true;
    editor.focus();
  }
  return {
    open,
    close,
    update,
    next: () => {
      if ($("#findbar").hidden) open();
      update(true);
    },
    previous: () => {
      if ($("#findbar").hidden) open();
      update(true, -1);
    },
  };
}
