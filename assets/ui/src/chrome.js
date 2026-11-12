import { $, $$, element, showError } from "./dom.js";
import { ICONS } from "./icons.js";
import { createSidebar } from "./sidebar.js";

export function createChrome(send, editor) {
  let tabsKey = "",
    outlineDocId = 0,
    outlineRef = null,
    lastError = "";
  const cleanPath = (p) => (p ? p.replace(/\\\\\?\\/g, "") : "");
  const sidebar = createSidebar(send);
  function accept(snapshot, doc = snapshot.document) {
    const { prefs } = snapshot;
    const root = document.documentElement;
    root.dataset.color = prefs.colorMode;
    root.dataset.mdTheme = prefs.mdTheme;
    if (document.activeElement !== $("#font-scale"))
      root.style.setProperty("--font-scale", prefs.fontScale);
    const isDark = prefs.colorMode === "dark";
    $("#theme-toggle").setAttribute("aria-pressed", String(isDark));
    $("#theme-toggle .btn-icon").innerHTML = isDark ? ICONS.sun : ICONS.moon;
    $("#theme-toggle-label").textContent = isDark ? "Light mode" : "Dark mode";
    $$(".segments button").forEach((button) =>
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.command === `view-${doc.viewMode}`),
      ),
    );

    const tab = snapshot.tabs.find((t) => t.id === doc.id);
    const activePath = tab?.path || "";
    document.title = `${tab?.dirty ? "• " : ""}${tab?.title || "Untitled"} — Markora`;
    $("#document-path").textContent = cleanPath(activePath) || "Unsaved document";
    $("#document-path").title = cleanPath(activePath);
    const nextTabsKey = JSON.stringify([snapshot.tabs, doc.id]);
    if (nextTabsKey !== tabsKey) {
      tabsKey = nextTabsKey;
      $("#tabs").replaceChildren(
        ...snapshot.tabs.map((t) => {
          const isActive = t.id === doc.id;
          const item = element("div", `tab${isActive ? " active" : ""}`);
          const button = element("button", "tab-title", t.title);
          button.type = "button";
          button.setAttribute("role", "tab");
          button.setAttribute("aria-selected", String(isActive));
          button.tabIndex = isActive ? 0 : -1;
          button.title = cleanPath(t.path) || t.title;
          button.onclick = () => send({ type: "activateTab", id: t.id });
          button.onauxclick = (event) => {
            if (event.button === 1) {
              event.preventDefault();
              send({ type: "closeTab", id: t.id });
            }
          };
          const close = element("button", "tab-close");
          close.type = "button";
          close.innerHTML = ICONS.x;
          close.setAttribute("aria-label", `Close ${t.title}`);
          close.onclick = () => send({ type: "closeTab", id: t.id });
          item.append(button);
          if (t.dirty) {
            const dot = element("span", "dirty-dot");
            dot.setAttribute("aria-label", "Unsaved changes");
            item.append(dot);
          }
          item.append(close);
          return item;
        }),
      );
      $(".tab.active")?.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
    $("#folder-path-label").textContent =
      cleanPath(snapshot.folder)?.split(/[\\/]/).filter(Boolean).at(-1) || "Open a folder…";
    $("#folder-path").title = cleanPath(snapshot.folder) || "Open folder";
    sidebar.accept(snapshot, activePath);
    if (outlineDocId !== doc.id || outlineRef !== doc.rendered.outline) {
      outlineDocId = doc.id;
      outlineRef = doc.rendered.outline;
      const frag = document.createDocumentFragment();
      for (const heading of doc.rendered.outline) {
        const button = element("button", "", heading.title || "Untitled heading");
        button.type = "button";
        button.dataset.level = String(heading.level);
        button.style.paddingLeft = `${8 + (heading.level - 1) * 12}px`;
        button.onclick = () => editor.jump(heading.start);
        frag.append(button);
      }
      $("#outline").replaceChildren(frag);
      if (!doc.rendered.outline.length)
        $("#outline").append(
          element(
            "p",
            "empty",
            doc.kind === "pdf" ? "Outline unavailable for PDFs." : "Headings appear here.",
          ),
        );
    }
    $("#status").textContent = cleanPath(snapshot.status);
    $("#status").classList.toggle("error", snapshot.error);
    if (snapshot.error && snapshot.status !== lastError) showError(cleanPath(snapshot.status));
    lastError = snapshot.error ? snapshot.status : "";
    const words = snapshot.wordCount ?? 0;
    const chars = snapshot.charCount ?? 0;
    $("#counts").textContent = `${words} ${words === 1 ? "word" : "words"} · ${chars} characters`;
    $("#save-state").textContent = tab?.dirty ? "Unsaved" : "Saved";
    $("#save-state").classList.toggle("dirty", !!tab?.dirty);
    const isPdfDoc = doc.kind === "pdf";
    $("#mode-status").textContent = isPdfDoc
      ? "PDF"
      : doc.viewMode[0].toUpperCase() + doc.viewMode.slice(1);
    $("#mode-status").dataset.mode = isPdfDoc ? "pdf" : doc.viewMode;
  }
  $("#tabs").addEventListener("keydown", (event) => {
    const tabs = $$('#tabs [role="tab"]');
    const current = tabs.indexOf(document.activeElement);
    if (current < 0) return;
    const move =
      event.key === "ArrowRight"
        ? 1
        : event.key === "ArrowLeft"
          ? -1
          : event.key === "Home"
            ? -current
            : event.key === "End"
              ? tabs.length - 1 - current
              : 0;
    if (!move && event.key !== "Home" && event.key !== "End") return;
    event.preventDefault();
    const next = tabs[(current + move + tabs.length) % tabs.length];
    next?.focus();
    next?.click();
  });
  $("#mode-status").onclick = () => {
    const mode = $("#mode-status").dataset.mode;
    if (mode === "pdf") return;
    const order = ["live", "source", "split"];
    const next = order[(order.indexOf(mode) + 1) % order.length];
    send({ type: "command", command: `view-${next}` });
  };
  $("#dismiss-error").onclick = () => {
    $("#error-banner").hidden = true;
  };
  $("#document").addEventListener("click", (event) => {
    const link = event.target.closest("a");
    if (link) {
      event.preventDefault();
      const url = link.getAttribute("href");
      if (/^(https?:\/\/|mailto:)/i.test(url)) send({ type: "openLink", url });
      else if (url && !url.startsWith("#"))
        send({ type: "openRelative", docId: editor.document().id, path: url });
    }
  });
  return { accept };
}
