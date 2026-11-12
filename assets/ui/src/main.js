import { showError } from "./dom.js";
import { hydrateIcons } from "./icons.js";
import { createBridge } from "./bridge.js";
import { createEditor } from "./editor.js";
import { createLayout } from "./layout.js";
import { createDialogs } from "./dialogs.js";
import { createCommands } from "./commands.js";
import { createChrome } from "./chrome.js";
import { createConflictDialog } from "./conflicts.js";

let snapshot = null;
const bridge = createBridge((message) => {
  if (message.type === "preparePrint") {
    const doc = editor.document();
    if (doc.id !== message.docId || doc.revision !== message.revision) {
      bridge.send({ type: "printReady", token: message.token });
      return;
    }
    editor.preparePrint();
    // Let the browser lay out the complete preview before native printing starts.
    document.fonts.ready.then(() => requestAnimationFrame(() => {
      bridge.send({ type: "printReady", token: message.token });
    }));
    return;
  }
  if (message.type === "requestClose") {
    bridge.send({ type: "command", command: "exit" });
    return;
  }
  if (message.type !== "state") return;
  snapshot = message.snapshot;
  if (snapshot.document) editor.accept(snapshot.document);
  layout.accept(snapshot.prefs);
  chrome.accept(snapshot, editor.document());
  dialogs.accept(snapshot);
  conflicts.accept(snapshot);
  commands.update();
});

const editor = createEditor(
  (delayMs) => bridge.scheduleEdit(delayMs),
  () => bridge.flushEdit(),
);
bridge.setDraft(() => editor.draft());
const preferences = (patch) => bridge.send({ type: "preferences", patch });
const layout = createLayout(preferences);
const dialogs = createDialogs(bridge.send, preferences, editor);
const conflicts = createConflictDialog(bridge.send);
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
