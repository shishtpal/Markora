import { $, $$, element, showError } from "./dom.js";
import { COMMANDS } from "./command-list.js";
import { createFind } from "./find.js";

const SEPARATORS = new Set([
  "save",
  "export",
  "close-tab",
  "exit",
  "find",
  "heading-1",
  "list",
  "toggle-files",
  "toggle-theme",
  "tab-next",
  "settings",
  "about",
]);

export function createCommands({ send, editor, dialogs, layout, getSnapshot, images, settings }) {
  const find = createFind(editor);
  const ids = new Set(COMMANDS.map((c) => c.id));
  const formatting = new Set(COMMANDS.filter((c) => c.menu === "Format").map((c) => c.id));
  const triggers = [];

  function closeMenus() {
    $$(".dropdown").forEach((node) => {
      node.hidden = true;
    });
    triggers.forEach((node) => node.setAttribute("aria-expanded", "false"));
  }
  function openMenu(index, focusItem = false) {
    closeMenus();
    const trigger = triggers[index];
    if (!trigger) return;
    triggers.forEach((t, i) => {
      t.tabIndex = i === index ? 0 : -1;
    });
    const dropdown = trigger.nextElementSibling;
    dropdown.hidden = false;
    trigger.setAttribute("aria-expanded", "true");
    if (focusItem) dropdown.querySelector("button")?.focus();
  }
  function run(id) {
    closeMenus();
    if (settings.handleCommand(id, formatting)) return;
    if (!editor.canEdit() && (formatting.has(id) || ["undo", "redo", "link", "replace", "find-toggle", "replace-current", "replace-all"].includes(id))) return;
    if (
      editor.isReading() &&
      (formatting.has(id) ||
        [
          "undo",
          "redo",
          "link",
          "replace",
          "find-toggle",
          "replace-current",
          "replace-all",
          "view-live",
          "view-source",
          "view-split",
          "toggle-focus",
          "toggle-files",
          "toggle-outline",
          "reset-layout",
        ].includes(id))
    )
      return;
    if (!ids.has(id)) {
      showError(`Unknown command: ${id}`);
      return;
    }
    if (editor.document().kind === "pdf") {
      if (id === "print") {
        $("#pdf-frame")?.contentWindow?.focus();
        $("#pdf-frame")?.contentWindow?.print();
        return;
      }
      if (id === "find" || id === "find-next" || id === "find-prev") {
        $("#pdf-frame")?.focus();
        $("#status").textContent = "Click inside the PDF or press Ctrl+F to search PDF text";
        return;
      }
      if (
        [
          "export",
          "undo",
          "redo",
          "link",
          "replace",
          "find-toggle",
          "replace-current",
          "replace-all",
        ].includes(id) ||
        formatting.has(id)
      ) {
        return;
      }
    }
    const local = {
      "command-palette": () => dialogs.palette(),
      "toggle-edit": () => { editor.toggleEdit(); find.update(); },
      settings: () => settings.show(),
      shortcuts: () => dialogs.help(),
      about: () => dialogs.about(),
      export: () => dialogs.exportDoc(),
      find: () => find.open(),
      replace: () => find.open(true),
      "find-toggle": () => find.toggleReplace(),
      "replace-current": () => find.replace(),
      "replace-all": () => find.replaceAll(),
      "find-next": () => find.next(),
      "find-prev": () => find.previous(),
      "find-close": () => find.close(),
      "reset-layout": () => layout.reset(),
      link: () => dialogs.link(),
      "insert-image": () => images.pick(),
      undo: () => editor.history("undo"),
      redo: () => editor.history("redo"),
    };
    if (local[id]) return local[id]();
    if (formatting.has(id)) return editor.format(id);
    send({ type: "command", command: id });
  }

  ["File", "Edit", "Format", "View", "Help"].forEach((name, menuIndex) => {
    const menu = element("div", "menu");
    const trigger = element("button", "menu-trigger", name);
    trigger.type = "button";
    trigger.tabIndex = menuIndex === 0 ? 0 : -1;
    trigger.setAttribute("aria-haspopup", "menu");
    trigger.setAttribute("aria-expanded", "false");
    const dropdown = element("div", "dropdown");
    dropdown.hidden = true;
    dropdown.setAttribute("role", "menu");
    dropdown.setAttribute("aria-label", name);
    for (const command of COMMANDS.filter((c) => c.menu === name)) {
      if (SEPARATORS.has(command.id) && dropdown.childElementCount > 0)
        dropdown.append(element("hr"));
      const button = element("button");
      button.type = "button";
      button.dataset.command = command.id;
      button.setAttribute("role", "menuitem");
      button.append(element("span", "", command.label));
      if (command.shortcut) button.append(element("kbd", "", command.shortcut));
      dropdown.append(button);
    }
    triggers.push(trigger);
    trigger.onclick = () => {
      if (dropdown.hidden) openMenu(menuIndex, false);
      else closeMenus();
    };
    trigger.onpointerenter = () => {
      if (triggers.some((t) => t !== trigger && t.getAttribute("aria-expanded") === "true")) {
        openMenu(menuIndex, false);
        trigger.focus({ preventScroll: true, focusVisible: false });
      }
    };
    trigger.onkeydown = (event) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        openMenu(menuIndex, true);
      } else if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
        event.preventDefault();
        const next =
          (menuIndex + (event.key === "ArrowRight" ? 1 : -1) + triggers.length) % triggers.length;
        const wasOpen = triggers.some((t) => t.getAttribute("aria-expanded") === "true");
        if (wasOpen) openMenu(next, false);
        else
          triggers.forEach((t, i) => {
            t.tabIndex = i === next ? 0 : -1;
          });
        triggers[next].focus({ preventScroll: true });
      }
    };
    dropdown.onkeydown = (event) => {
      const buttons = [...dropdown.querySelectorAll("button")];
      const current = buttons.indexOf(document.activeElement);
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        buttons[
          (current + (event.key === "ArrowDown" ? 1 : buttons.length - 1)) % buttons.length
        ]?.focus();
      } else if (event.key === "Home" || event.key === "End") {
        event.preventDefault();
        buttons[event.key === "Home" ? 0 : buttons.length - 1]?.focus();
      } else if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
        event.preventDefault();
        const next =
          (menuIndex + (event.key === "ArrowRight" ? 1 : -1) + triggers.length) % triggers.length;
        openMenu(next, true);
      } else if (event.key === "Escape") {
        event.stopPropagation();
        closeMenus();
        trigger.focus();
      } else if (event.key === "Tab") {
        closeMenus();
      }
    };
    menu.append(trigger, dropdown);
    $("#menus").append(menu);
  });

  if (!("focusGroup" in HTMLElement.prototype)) {
    const bar = $("#formatbar");
    const items = [...bar.querySelectorAll("button")];
    items.forEach((b, i) => {
      b.tabIndex = i ? -1 : 0;
    });
    bar.addEventListener("focusin", ({ target }) => {
      if (items.includes(target))
        items.forEach((b) => {
          b.tabIndex = b === target ? 0 : -1;
        });
    });
    bar.addEventListener("keydown", (event) => {
      const i = items.indexOf(document.activeElement);
      if (i < 0) return;
      const move =
        event.key === "ArrowRight"
          ? 1
          : event.key === "ArrowLeft"
            ? -1
            : event.key === "Home"
              ? -i
              : event.key === "End"
                ? items.length - 1 - i
                : 0;
      if (!move && event.key !== "Home" && event.key !== "End") return;
      event.preventDefault();
      items[(i + move + items.length) % items.length].focus();
    });
  }

  document.addEventListener("mousedown", (event) => {
    if (event.target.closest("[data-command], .menu-trigger")) event.preventDefault();
  });
  document.addEventListener("click", (event) => {
    const button = event.target.closest("[data-command]");
    if (button) run(button.dataset.command);
    else if (!event.target.closest(".menu")) closeMenus();
  });
  function readingKey(key) {
    if (key === "F11") return run("toggle-reading");
    if (key !== "Escape") return;
    const dialog = $("dialog[open]");
    if (dialog) {
      if (dialog.dispatchEvent(new Event("cancel", { cancelable: true }))) dialog.close();
    } else if (settings.active()) settings.close();
    else if (!$("#findbar").hidden) find.close();
    else if (editor.isReading()) run("exit-reading");
  }
  document.addEventListener("keydown", (event) => {
    if (event.isComposing || event.defaultPrevented || event.altKey) return;
    if (event.key === "F11" && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      if (!event.repeat) readingKey("F11");
      return;
    }
    if ($("dialog[open]")) return;
    const mod = event.ctrlKey || event.metaKey;
    if (
      mod &&
      event.target.closest("#findbar input, #settings-page input") &&
      ["z", "y"].includes(event.key.toLowerCase())
    )
      return;
    const prefix = `${mod ? "Ctrl+" : ""}${event.shiftKey ? "Shift+" : ""}`;
    const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
    let command = COMMANDS.find(
      (c) => c.shortcut.toLowerCase() === (prefix + key).toLowerCase(),
    )?.id;
    if (mod && key === "T" && !event.shiftKey) command = "new-tab";
    if (mod && key === "K") command = "command-palette";
    if (mod && key === "Y") command = "redo";
    if (event.key === "Escape") {
      closeMenus();
      if (settings.active()) { event.preventDefault(); return settings.close(); }
      if (!$("#findbar").hidden) return find.close();
      if (editor.isReading()) command = "exit-reading";
      else if (getSnapshot()?.prefs.focusMode) command = "toggle-focus";
    }
    if (command) {
      event.preventDefault();
      run(command);
    }
  });
  dialogs.setRun(run);
  return { run, readingKey, update: () => find.update() };
}
