import { $, $$ } from "./dom.js";
import { createSearchRunner } from "./search-worker.js";

export function createFind(editor) {
  const bar = $("#findbar"),
    input = $("#find-input"),
    replacement = $("#replace-input");
  const count = $("#find-count"),
    message = $("#find-message");
  const runner = createSearchRunner();
  let key = null,
    pendingKey = null,
    pending = null,
    timer = null;
  let hits = [],
    index = -1,
    anchor = null,
    limited = false,
    busy = false;
  let failed = false,
    replacing = false,
    generation = 0,
    lastDoc = null;

  function request() {
    const doc = editor.document();
    return {
      text: doc.markdown,
      query: input.value,
      caseSensitive: $("#find-case").getAttribute("aria-pressed") === "true",
      wholeWord: $("#find-word").getAttribute("aria-pressed") === "true",
      regex: $("#find-regex").getAttribute("aria-pressed") === "true",
    };
  }
  function stamp() {
    const doc = editor.document(),
      options = request();
    return JSON.stringify([
      doc.id,
      doc.revision,
      options.query,
      options.caseSensitive,
      options.wholeWord,
      options.regex,
    ]);
  }
  function paint() {
    count.textContent = busy
      ? "Searching…"
      : input.value
        ? `${index >= 0 ? index + 1 : 0} / ${hits.length}${limited ? "+" : ""}`
        : "";
    count.classList.toggle("no-matches", Boolean(input.value) && !busy && !hits.length);
    input.setAttribute("aria-invalid", String(failed));
    for (const id of ["replace-one", "replace-all"])
      $("#" + id).disabled =
        busy || replacing || failed || !hits.length || (id === "replace-all" && limited);
  }
  function stop() {
    clearTimeout(timer);
    timer = null;
    generation++;
    runner.cancel();
    pendingKey = null;
    pending = null;
    busy = false;
  }
  async function ensure() {
    clearTimeout(timer);
    timer = null;
    if (bar.hidden || editor.document().kind === "pdf") return false;
    const nextKey = stamp();
    if (key === nextKey) return !failed;
    if (pendingKey === nextKey) return pending;
    const docId = editor.document().id;
    if (lastDoc !== docId) {
      anchor = null;
      index = -1;
      lastDoc = docId;
    }
    const token = ++generation;
    pendingKey = nextKey;
    busy = true;
    failed = false;
    message.textContent = "";
    paint();
    pending = runner.run(request()).then((result) => {
      if (token !== generation || bar.hidden) return false;
      pendingKey = null;
      pending = null;
      busy = false;
      if (stamp() !== nextKey) {
        update();
        return false;
      }
      if (!result) return false;
      key = nextKey;
      hits = result.hits || [];
      limited = Boolean(result.limited);
      failed = Boolean(result.error);
      index = anchor === null || !hits.length ? -1 : hits.findIndex((hit) => hit.start >= anchor);
      if (anchor !== null && hits.length && index < 0) index = 0;
      message.textContent =
        result.error ||
        (limited ? "Showing the first 100,000 matches. Refine the search to replace all." : "");
      paint();
      return !failed;
    });
    return pending;
  }

  function update() {
    if (bar.hidden || editor.document().kind === "pdf") {
      if (pending || replacing) stop();
      return;
    }
    if (!replacing) void ensure();
  }

  function changed() {
    stop();
    key = null;
    anchor = null;
    hits = [];
    index = -1;
    failed = false;
    message.textContent = "";
    busy = true;
    paint();
    timer = setTimeout(update, 100);
  }
  async function navigate(direction) {
    if (bar.hidden) open();
    if (!(await ensure()) || !hits.length || replacing) return;
    index =
      index < 0
        ? direction > 0
          ? 0
          : hits.length - 1
        : (index + direction + hits.length) % hits.length;
    const hit = hits[index];
    anchor = hit.start;
    paint();
    editor.jump(hit.start, hit.length);
  }
  async function replace(all = false) {
    if (replacing) return;
    const ready = await ensure();
    if (replacing || !ready || !hits.length || (all && limited)) return;
    const before = stamp(),
      focus = document.activeElement,
      token = ++generation;
    replacing = true;
    paint();
    const result = await runner.run({
      ...request(),
      operation: all ? "replaceAll" : "replace",
      replacement: replacement.value,
      index: Math.max(0, index),
    });
    replacing = false;
    if (token !== generation || bar.hidden) {
      paint();
      return;
    }
    if (stamp() !== before) {
      key = null;
      update();
      return;
    }
    if (!result || result.error) {
      message.textContent = result?.error || "Replacement cancelled.";
      paint();
      return;
    }
    if (result.text !== undefined) editor.replaceDocument(result.text);
    key = null;
    anchor = all ? null : result.anchor;
    if (await ensure()) {
      if (!all && index >= 0) editor.jump(hits[index].start, hits[index].length);
      message.textContent = `Replaced ${result.replaced} ${result.replaced === 1 ? "match" : "matches"}.`;
      if (bar.contains(focus)) focus.focus({ preventScroll: true });
    }
  }
  function toggleReplace(show = $("#replace-row").hidden) {
    $("#replace-row").hidden = !show;
    $("#find-toggle").setAttribute("aria-expanded", String(show));
    if (show) replacement.focus();
  }
  function open(showReplace = false) {
    if (editor.document().kind === "pdf") return;
    bar.hidden = false;
    toggleReplace(showReplace);
    input.focus();
    input.select();
    update();
  }
  function close() {
    stop();
    bar.hidden = true;
    editor.focus();
  }

  input.addEventListener("input", changed);
  replacement.addEventListener("input", () => {
    if (replacing) {
      stop();
      replacing = false;
    }
    paint();
  });

  for (const button of $$("[data-find-option]"))
    button.onclick = () => {
      button.setAttribute("aria-pressed", String(button.getAttribute("aria-pressed") !== "true"));
      changed();
    };
  bar.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
    }
    if (event.key === "Enter" && (event.target === input || event.target === replacement)) {
      event.preventDefault();
      event.stopPropagation();
      if (event.target === replacement) void replace(event.ctrlKey || event.metaKey);
      else void navigate(event.shiftKey ? -1 : 1);
    }
  });
  window.addEventListener("pagehide", () => runner.destroy());
  return {
    open,
    close,
    update,
    toggleReplace,
    replace: () => replace(),
    replaceAll: () => replace(true),
    next: () => navigate(1),
    previous: () => navigate(-1),
  };
}
