import { $, $$ } from "./dom.js";
import { applyTypography } from "./typography.js";

// A transient workspace tab; document drafts and tab ownership stay with Rust.
export function createSettings(send, preferences, editor) {
  const page = $("#settings-page");
  const scale = $("#font-scale");
  const size = $("#font-size");
  let opened = false, active = false, docId = null, focus = null;
  let selection = null, scroll = [];
  let changed = () => {};
  let prefs = {};

  function scaleDisplay(value) {
    const pct = `${Math.round(Number(value) * 100)}%`;
    $("#font-scale-value").textContent = pct;
    scale.style.setProperty("--control-value", value);
    scale.setAttribute("aria-valuetext", pct);
  }
  function paint() {
    page.hidden = !active;
    document.body.classList.toggle("settings-open", active);
    for (const selector of ["#viewbar", "#findbar", "#workspace", "#statusbar"])
      $(selector).inert = active;
    changed();
  }
  function show() {
    if (!active) {
      focus = document.activeElement;
      selection = focus instanceof HTMLTextAreaElement
        ? [focus.selectionStart, focus.selectionEnd, focus.selectionDirection] : null;
      scroll = [$("#source-editor"), $("#live-pane"), $("#preview-pane")]
        .map((node) => [node, node.scrollTop, node.scrollLeft]);
      docId = editor.document().id;
      editor.capturePosition();
      send({ type: "edit" });
    }
    opened = active = true;
    paint();
    $("#settings-title").focus({ preventScroll: true });
  }
  function leave(restore = true) {
    if (!active) return;
    active = false;
    paint();
    editor.reflow();
    if (restore && editor.document().id === docId) {
      if (editor.isReading()) $("#preview-pane").focus({ preventScroll: true });
      else if (focus?.isConnected && focus.getClientRects().length) {
        focus.focus({ preventScroll: true });
        if (selection) focus.setSelectionRange(...selection);
      } else editor.focus();
      for (const [node, top, left] of scroll) { node.scrollTop = top; node.scrollLeft = left; }
    }
  }
  function close() {
    opened = false;
    leave();
    changed();
  }
  function handleCommand(id, formatting) {
    if (active) {
      if (id === "close-tab") { close(); return true; }
      if (id === "tab-next" || id === "tab-prev") {
        const tabs = $$('#tabs [role="tab"]:not(#settings-tab)');
        (id === "tab-next" ? tabs[0] : tabs.at(-1))?.click();
        return true;
      }
      if (formatting.has(id) || ["undo", "redo", "link", "insert-image", "toggle-edit", "find", "replace",
        "find-toggle", "find-next", "find-prev", "replace-current", "replace-all"].includes(id)) return true;
      if (["new-tab", "open", "open-folder", "close-all", "view-live", "view-source", "view-split",
        "toggle-reading", "exit-reading", "print", "export", "export-html", "export-pdf",
        "export-rtf", "export-txt"].includes(id)) leave(false);
    } else if (opened && (id === "tab-next" || id === "tab-prev")) {
      const tabs = $$('#tabs [role="tab"]');
      const index = tabs.findIndex((tab) => tab.getAttribute("aria-selected") === "true");
      tabs[(index + (id === "tab-next" ? 1 : tabs.length - 1)) % tabs.length]?.click();
      return true;
    }
    return false;
  }
  $("#close-settings").onclick = close;
  page.addEventListener("scroll", () => {
    const links = $$(".settings-nav a");
    const top = page.getBoundingClientRect().top + 64;
    const current = links.findLast((link) => $(link.hash).getBoundingClientRect().top <= top) || links[0];
    links.forEach((link) => {
      if (link === current) link.setAttribute("aria-current", "location");
      else link.removeAttribute("aria-current");
    });
  }, { passive: true });
  $$(".settings-nav a").forEach((link) => {
    link.onclick = (event) => {
      event.preventDefault();
      $(link.hash).scrollIntoView({ block: "start" });
      $$(".settings-nav a").forEach((item) => item.removeAttribute("aria-current"));
      link.setAttribute("aria-current", "location");
    };
  });
  $("#color-mode").onchange = (e) => preferences({ colorMode: e.target.value });
  $("#md-theme").onchange = (e) => preferences({ mdTheme: e.target.value });
  $("#font-family").onchange = (e) => {
    prefs = { ...prefs, fontFamily: e.target.value };
    applyTypography(prefs);
    preferences({ fontFamily: e.target.value });
  };
  size.oninput = () => {
    if (!size.value || !size.validity.valid) return;
    prefs = { ...prefs, fontSize: size.valueAsNumber };
    applyTypography(prefs);
  };
  size.onchange = () => {
    const value = size.value ? Math.round(Math.max(12, Math.min(32, size.valueAsNumber))) : prefs.fontSize;
    size.value = value;
    prefs = { ...prefs, fontSize: value };
    applyTypography(prefs);
    preferences({ fontSize: value });
  };
  scale.oninput = () => {
    document.documentElement.style.setProperty("--font-scale", scale.value);
    scaleDisplay(scale.value);
  };
  scale.onchange = () => preferences({ fontScale: Number(scale.value) });
  $("#reset-typography").onclick = () => preferences({ fontFamily: "theme", fontSize: 16, fontScale: 1 });
  $("#asset-mode").onchange = (e) => preferences({ assetMode: e.target.value });
  $("#asset-custom-path").onchange = (e) => preferences({ assetCustomPath: e.target.value });
  $("#asset-folder-browse").onclick = () => send({ type: "pickAssetFolder" });
  $("#show-files").onchange = (e) => preferences({ showFiles: e.target.checked });
  $("#show-outline").onchange = (e) => preferences({ showOutline: e.target.checked });

  return {
    show, leave, close, handleCommand,
    active: () => active,
    opened: () => opened,
    onChange: (handler) => { changed = handler; },
    accept(snapshot) {
      if (active && snapshot.activeDocId !== docId) leave(false);
      prefs = snapshot.prefs;
      applyTypography(document.activeElement === size && size.value && size.validity.valid
        ? { ...prefs, fontSize: size.valueAsNumber } : prefs);
      for (const [id, key] of [["color-mode", "colorMode"], ["md-theme", "mdTheme"],
        ["asset-mode", "assetMode"], ["asset-custom-path", "assetCustomPath"],
        ["font-family", "fontFamily"], ["font-size", "fontSize"], ["font-scale", "fontScale"]]) {
        if (document.activeElement !== $(`#${id}`)) $(`#${id}`).value = prefs[key];
      }
      $("#asset-custom-settings").hidden = prefs.assetMode !== "custom";
      $("#show-files").checked = prefs.showFiles;
      $("#show-outline").checked = prefs.showOutline;
      scaleDisplay(document.activeElement === scale ? scale.value : prefs.fontScale);
    },
  };
}
