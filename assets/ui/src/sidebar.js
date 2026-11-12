import { $, element } from "./dom.js";
import { ICONS } from "./icons.js";

export function createSidebar(send) {
  let folder,
    entries = [],
    activePath = "",
    key = "",
    recentsKey = "",
    focusPath = "";
  const expanded = new Set();
  const requested = new Set();
  const tree = $("#files");
  $("#toggle-recents").onclick = () => {
    const hidden = !$("#recents").hidden;
    $("#recents").hidden = hidden;
    $("#toggle-recents").setAttribute("aria-expanded", String(!hidden));
  };
  function fileButton(name, path, isFolder) {
    const button = element("button");
    button.type = "button";
    button.dataset.path = path;
    button.title = path.replace(/\\\\\?\\/g, "");
    if (!isFolder && path === activePath) {
      button.classList.add("active");
      button.setAttribute("aria-current", "page");
    }
    const icon = element("span", `file-icon${isFolder ? " is-folder" : ""}`);
    icon.innerHTML = ICONS[isFolder ? "folder" : /\.pdf$/i.test(name) ? "filePdf" : "file"];
    button.append(icon, element("span", "", name));
    button.onclick = () => send({ type: "openPath", path });
    return button;
  }
  function toggle(path) {
    if (expanded.has(path)) expanded.delete(path);
    else {
      expanded.add(path);
      requested.delete(path);
    }
    focusPath = path;
    render();
  }
  function render() {
    const focused = tree.contains(document.activeElement);
    if (focused) focusPath = document.activeElement.dataset.path || focusPath;
    let groupId = 0;
    function branch(items, level = 1, parent = "") {
      const fragment = document.createDocumentFragment();
      items.forEach((entry, index) => {
        const button = fileButton(entry.name, entry.path, entry.isFolder);
        button.setAttribute("role", "treeitem");
        button.setAttribute("aria-label", entry.name);
        button.setAttribute("aria-level", level);
        button.setAttribute("aria-posinset", index + 1);
        button.setAttribute("aria-setsize", items.length);
        button.dataset.parent = parent;
        button.style.paddingLeft = `${8 + (level - 1) * 16}px`;
        button.tabIndex = -1;
        const chevron = element("span", "tree-chevron", entry.isFolder ? "›" : "");
        chevron.setAttribute("aria-hidden", "true");
        button.prepend(chevron);
        fragment.append(button);
        if (!entry.isFolder) return;
        const open = expanded.has(entry.path);
        button.setAttribute("aria-expanded", String(open));
        button.onclick = () => toggle(entry.path);
        if (!open) return;
        const group = element("div", "tree-group");
        group.id = `file-group-${++groupId}`;
        group.setAttribute("role", "group");
        button.setAttribute("aria-owns", group.id);
        fragment.append(group);
        if (entry.children) group.append(branch(entry.children, level + 1, entry.path));
        if (entry.children?.length === 0)
          group.append(element("p", "tree-note", "No supported files."));
        if (!entry.children) {
          group.append(
            element(
              "p",
              "tree-note",
              entry.error
                ? `Could not load folder: ${entry.error}. Collapse and expand to retry.`
                : "Loading…",
            ),
          );
          if (!entry.loading && !requested.has(entry.path)) {
            requested.add(entry.path);
            queueMicrotask(() => send({ type: "loadFolder", path: entry.path }));
          }
        }
      });
      return fragment;
    }
    tree.replaceChildren(branch(entries));
    if (!entries.length)
      tree.append(
        element(
          "p",
          "empty",
          folder ? "No supported files here." : "Open a folder to browse files.",
        ),
      );
    const rows = [...tree.querySelectorAll('[role="treeitem"]')];
    const target =
      rows.find((b) => b.dataset.path === focusPath) ||
      rows.find((b) => b.classList.contains("active")) ||
      rows[0];
    if (target) {
      target.tabIndex = 0;
      if (focused) target.focus({ preventScroll: true });
    }
  }
  tree.addEventListener("focusin", (event) => {
    if (!event.target.matches('[role="treeitem"]')) return;
    focusPath = event.target.dataset.path;
    for (const row of tree.querySelectorAll('[role="treeitem"]'))
      row.tabIndex = row === event.target ? 0 : -1;
  });
  tree.addEventListener("keydown", (event) => {
    const row = event.target.closest('[role="treeitem"]');
    if (!row) return;
    const rows = [...tree.querySelectorAll('[role="treeitem"]')];
    const index = rows.indexOf(row),
      open = row.getAttribute("aria-expanded");
    let next;
    switch (event.key) {
      case "ArrowDown":
        next = rows[Math.min(index + 1, rows.length - 1)];
        break;
      case "ArrowUp":
        next = rows[Math.max(0, index - 1)];
        break;
      case "Home":
        next = rows[0];
        break;
      case "End":
        next = rows.at(-1);
        break;
      case "ArrowRight":
        if (open === "false") toggle(row.dataset.path);
        else if (open === "true" && rows[index + 1]?.dataset.parent === row.dataset.path)
          next = rows[index + 1];
        break;
      case "ArrowLeft":
        if (open === "true") toggle(row.dataset.path);
        else next = rows.find((b) => b.dataset.path === row.dataset.parent);
        break;
      default:
        return;
    }
    event.preventDefault();
    next?.focus();
  });
  return {
    accept(snapshot, path) {
      const next = JSON.stringify([snapshot.folder, snapshot.entries, path]);
      activePath = path;
      if (next !== key) {
        if (folder !== snapshot.folder) {
          expanded.clear();
          requested.clear();
          focusPath = "";
        }
        folder = snapshot.folder;
        entries = snapshot.entries;
        key = next;
        // A response or refresh permits retries for unloaded branches.
        requested.clear();
        function failed(items) {
          for (const entry of items) {
            if (entry.error) requested.add(entry.path);
            if (entry.children) failed(entry.children);
          }
        }
        failed(entries);
        render();
      }
      const recentKey = JSON.stringify([snapshot.recents, path]);
      if (recentKey === recentsKey) return;
      recentsKey = recentKey;
      $("#recents").replaceChildren(
        ...snapshot.recents.map((r) => fileButton(r.path.split(/[\\/]/).at(-1), r.path, false)),
      );
      if (!snapshot.recents.length)
        $("#recents").append(element("p", "empty", "Opened files appear here."));
    },
  };
}
