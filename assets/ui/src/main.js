import { showError } from "./dom.js";
import { hydrateIcons } from "./icons.js";
import { createBridge } from "./bridge.js";
import { createEditor } from "./editor.js";
import { createLayout } from "./layout.js";
import { createDialogs } from "./dialogs.js";
import { createCommands } from "./commands.js";
import { createChrome } from "./chrome.js";
import { createConflictDialog } from "./conflicts.js";
import { createReading } from "./reading.js";
import { createImages } from "./images.js";
import { createSettings } from "./settings.js";

let snapshot = null;
const bridge = createBridge((message) => {
  if (message.type === "fileDrop") {
    images.nativeDrop(message.paths, message.position);
    return;
  }
  if (message.type === "readingKey") {
    commands.readingKey(message.key);
    return;
  }
  if (message.type === "preparePrint") {
    const doc = editor.document();
    if (doc.id !== message.docId || doc.revision !== message.revision) {
      bridge.send({ type: "printReady", token: message.token });
      return;
    }
    editor.preparePrint();
    // Let the browser lay out the complete preview before native printing starts.
    document.fonts.ready.then(() =>
      requestAnimationFrame(() => {
        bridge.send({ type: "printReady", token: message.token });
      }),
    );
    return;
  }
  if (message.type === "requestClose") {
    bridge.send({ type: "command", command: "exit" });
    return;
  }
  if (message.type !== "state") return;
  snapshot = message.snapshot;
  reading.before(snapshot.reading);
  if (snapshot.document) editor.accept(snapshot.document);
  editor.retainTabs(snapshot.tabs);
  layout.accept(snapshot.prefs);
  settings.accept(snapshot);
  chrome.accept(snapshot, editor.document());
  reading.accept(snapshot.tabs);
  dialogs.accept(snapshot);
  conflicts.accept(snapshot);
  commands.update();
  images.accept(snapshot);
});

const editor = createEditor(
  (delayMs) => bridge.scheduleEdit(delayMs),
  () => bridge.flushEdit(),
);
bridge.setDraft(() => editor.draft());
const reading = createReading(editor);
editor.setReader(reading);
const preferences = (patch) => bridge.send({ type: "preferences", patch });
const layout = createLayout(preferences);
const dialogs = createDialogs(bridge.send, editor);
const settings = createSettings(bridge.send, preferences, editor);
const conflicts = createConflictDialog(bridge.send);
const images = createImages(bridge, editor, () => snapshot);
const commands = createCommands({
  send: bridge.send,
  editor,
  dialogs,
  layout,
  getSnapshot: () => snapshot,
  images,
  settings,
});
const chrome = createChrome(bridge.send, editor, settings);
window.addEventListener("error", (event) => showError(event.message));
window.addEventListener("unhandledrejection", (event) =>
  showError(event.reason?.message || event.reason),
);
hydrateIcons();
bridge.start();
