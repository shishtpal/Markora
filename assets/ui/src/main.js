import { showError } from "./dom.js";
import { hydrateIcons } from "./icons.js";
import { createBridge } from "./bridge.js";
import { createEditor } from "./editor.js";
import { createLayout } from "./layout.js";
import { createDialogs } from "./dialogs.js";
import { createCommands } from "./commands.js";
import { createChrome } from "./chrome.js";

let snapshot = null;
const bridge = createBridge((message) => {
  if (message.type === "requestClose") {
    bridge.send({ type: "command", command: "exit" });
    return;
  }
  if (message.type !== "state") return;
  snapshot = message.snapshot;
  editor.accept(snapshot.document);
  layout.accept(snapshot.prefs);
  chrome.accept(snapshot);
  dialogs.accept(snapshot);
  commands.update();
});

const editor = createEditor(() => bridge.send({ type: "edit" }));
bridge.setDraft(() => editor.draft());
const preferences = (patch) => bridge.send({ type: "preferences", patch });
const layout = createLayout(preferences);
const dialogs = createDialogs(bridge.send, preferences, editor);
const commands = createCommands({
  send: bridge.send,
  editor,
  dialogs,
  layout,
  getSnapshot: () => snapshot,
});
const chrome = createChrome(bridge.send, editor);
window.addEventListener("error", (event) => showError(event.message));
window.addEventListener("unhandledrejection", (event) =>
  showError(event.reason?.message || event.reason),
);
hydrateIcons();
bridge.start();
