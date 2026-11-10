import { $, $$, element } from "./dom.js";
import { COMMANDS } from "./command-list.js";

// Native HTML dialogs provide focus containment, Escape and focus restoration.
export function createDialogs(send, preferences, editor) {
  let run = () => {};
  let selected = 0;
  let hits = [];
  let pendingTitle = null;
  const palette = $("#palette-dialog");
  const unsaved = $("#unsaved-dialog");
  const scaleInput = $("#font-scale");

  function show(dialog) {
    if (!dialog.open) dialog.showModal();
  }
  function syncScaleDisplay(value) {
    const pct = `${Math.round(Number(value) * 100)}%`;
    $("#font-scale-value").textContent = pct;
    scaleInput.style.setProperty("--control-value", value);
    scaleInput.setAttribute("aria-valuetext", pct);
  }
  function renderPalette() {
    const query = $("#palette-input").value.toLowerCase().trim();
    hits = COMMANDS.filter((c) => c.menu && `${c.label} ${c.id} ${c.menu}`.toLowerCase().includes(query));
    selected = Math.max(0, Math.min(selected, hits.length - 1));
    $("#palette-list").replaceChildren(
      ...hits.map((command, index) => {
        const button = element("button", index === selected ? "selected" : "");
        button.type = "button";
        button.setAttribute("role", "option");
        button.setAttribute("aria-selected", String(index === selected));
        const main = element("span", "palette-item-main");
        main.append(element("span", "palette-cat", command.menu), element("span", "", command.label));
        button.append(main);
        if (command.shortcut) button.append(element("kbd", "", command.shortcut));
        button.onclick = () => {
          palette.close();
          run(command.id);
        };
        return button;
      }),
    );
    if (!hits.length) $("#palette-list").append(element("p", "empty", "No commands match."));
    $("#palette-list .selected")?.scrollIntoView({ block: "nearest" });
  }
  $("#palette-input").addEventListener("input", () => {
    selected = 0;
    renderPalette();
  });
  $("#palette-input").addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      selected += event.key === "ArrowDown" ? 1 : -1;
      renderPalette();
    }
    if (event.key === "Enter" && hits[selected]) {
      event.preventDefault();
      palette.close();
      run(hits[selected].id);
    }
  });
  $$("[data-close-dialog]").forEach(
    (button) => (button.onclick = () => button.closest("dialog").close()),
  );

  if (!("closedBy" in HTMLDialogElement.prototype)) {
    const outside = (e, dialog) => {
      const r = dialog.getBoundingClientRect();
      return e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom;
    };
    $$("dialog").forEach((dialog) => {
      if (dialog === unsaved) return;
      let pressedOutside = false;
      dialog.addEventListener("pointerdown", (e) => {
        pressedOutside = e.target === dialog && outside(e, dialog);
      });
      dialog.addEventListener("pointerup", (e) => {
        if (pressedOutside && e.target === dialog && outside(e, dialog)) dialog.close();
        pressedOutside = false;
      });
    });
  }

  $$("[data-unsaved]").forEach(
    (button) =>
      (button.onclick = () => send({ type: "resolveUnsaved", choice: button.dataset.unsaved })),
  );
  unsaved.addEventListener("cancel", (event) => {
    event.preventDefault();
    send({ type: "resolveUnsaved", choice: "cancel" });
  });
  $("#color-mode").onchange = (event) => preferences({ colorMode: event.target.value });
  $("#md-theme").onchange = (event) => preferences({ mdTheme: event.target.value });
  $("#show-files").onchange = (event) => preferences({ showFiles: event.target.checked });
  $("#show-outline").onchange = (event) => preferences({ showOutline: event.target.checked });
  scaleInput.oninput = (event) => {
    document.documentElement.style.setProperty("--font-scale", event.target.value);
    syncScaleDisplay(event.target.value);
  };
  scaleInput.onchange = (event) => preferences({ fontScale: Number(event.target.value) });
  $("#link-form").onsubmit = (event) => {
    event.preventDefault();
    const url = $("#link-url").value;
    if (!/^https?:\/\//i.test(url)) {
      $("#link-url").setCustomValidity("Enter an http:// or https:// URL.");
      $("#link-url").reportValidity();
      return;
    }
    $("#link-dialog").close();
    editor.format("link", url);
  };
  $("#link-url").oninput = () => $("#link-url").setCustomValidity("");
  const updateExportOptions = () => {
    const selectedFormat = document.querySelector('input[name="export-format"]:checked')?.value;
    $("#export-html-options").hidden = selectedFormat !== "html";
  };
  $$('input[name="export-format"]').forEach((input) => (input.onchange = updateExportOptions));
  $("#export-form").onsubmit = (event) => {
    event.preventDefault();
    const format = document.querySelector('input[name="export-format"]:checked')?.value || "html";
    const htmlStyle = $("#export-html-style").value;
    $("#export-dialog").close();
    send({ type: "export", format, htmlStyle });
  };

  const categories = ["File", "Edit", "Format", "View", "Help"];
  $("#help-list").replaceChildren(
    ...categories
      .map((cat) => {
        const items = COMMANDS.filter((c) => c.menu === cat && c.shortcut);
        if (!items.length) return null;
        const group = element("section", "help-group");
        group.append(element("h3", "", cat));
        for (const c of items) {
          const row = element("div", "help-row");
          row.append(element("span", "", c.label), element("kbd", "", c.shortcut));
          group.append(row);
        }
        return group;
      })
      .filter(Boolean),
  );

  return {
    setRun: (handler) => { run = handler; },
    palette() {
      selected = 0;
      $("#palette-input").value = "";
      renderPalette();
      show(palette);
      $("#palette-input").focus();
    },
    settings() { show($("#settings-dialog")); },
    help() { show($("#help-dialog")); },
    about() { show($("#about-dialog")); },
    exportDoc() {
      updateExportOptions();
      show($("#export-dialog"));
    },
    link() {
      $("#link-url").value = "";
      show($("#link-dialog"));
      $("#link-url").focus();
    },
    accept(snapshot) {
      const prefs = snapshot.prefs;
      $("#color-mode").value = prefs.colorMode;
      $("#md-theme").value = prefs.mdTheme;
      $("#show-files").checked = prefs.showFiles;
      $("#show-outline").checked = prefs.showOutline;
      if (document.activeElement !== scaleInput) scaleInput.value = prefs.fontScale;
      syncScaleDisplay(prefs.fontScale);
      if (snapshot.pending) {
        $("#unsaved-description").textContent =
          `Save changes to “${snapshot.pending}” before closing?`;
        if (pendingTitle !== snapshot.pending)
          $$("dialog[open]").forEach((d) => { if (d !== unsaved) d.close(); });
        show(unsaved);
      } else if (unsaved.open) unsaved.close();
      pendingTitle = snapshot.pending;
    },
  };
}
