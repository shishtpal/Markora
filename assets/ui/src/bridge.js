import { $, showError } from "./dom.js";

// One transport, a startup handshake, and a fresh draft on every outgoing action.
export function createBridge(receive) {
  const queue = [];
  let draft = () => null;
  let connected = false;
  let retryTimer = null;
  let editTimer = null;
  const BACKOFF = [15, 30, 60, 120, 200];

  function flush() {
    const transport =
      window.ipc?.postMessage?.bind(window.ipc) ||
      window.chrome?.webview?.postMessage?.bind(window.chrome.webview);
    if (!transport) return;
    while (queue.length) {
      try {
        transport(queue[0]);
        queue.shift();
      } catch (error) {
        showError(`Could not send command: ${error.message}`);
        break;
      }
    }
  }
  function send(action) {
    if (editTimer) {
      clearTimeout(editTimer);
      editTimer = null;
    }
    queue.push(JSON.stringify({ ...action, draft: draft() }));
    flush();
  }
  function scheduleEdit(delayMs = 0) {
    if (delayMs <= 0) {
      send({ type: "edit" });
      return;
    }
    if (editTimer) clearTimeout(editTimer);
    editTimer = setTimeout(() => {
      editTimer = null;
      send({ type: "edit" });
    }, delayMs);
  }
  function flushEdit() {
    if (editTimer) send({ type: "edit" });
  }

  window.markoraReceive = (message) => {
    if (message.type === "state") {
      connected = true;
      if (retryTimer) {
        clearTimeout(retryTimer);
        retryTimer = null;
      }
      $("#connection").textContent = "Local";
      $("#connection").classList.add("connected");
      $("#connection").classList.remove("error");
    }
    try {
      receive(message);
    } catch (error) {
      showError(`Interface error: ${error.message}`);
      console.error(error);
    }
  };
  function start() {
    send({ type: "ready" });
    let step = 0;
    const poll = () => {
      if (connected) return;
      flush();
      if (!connected && queue.length === 0) send({ type: "ready" });
      const delay = BACKOFF[Math.min(step++, BACKOFF.length - 1)];
      retryTimer = setTimeout(poll, delay);
    };
    retryTimer = setTimeout(poll, BACKOFF[0]);
    setTimeout(() => {
      if (connected) return;
      $("#connection").textContent = "Disconnected";
      $("#connection").classList.remove("connected");
      $("#connection").classList.add("error");
      showError(
        "The native host has not connected. Restart Markora; queued commands will run once connected.",
      );
    }, 6000);
  }
  return {
    send,
    scheduleEdit,
    flushEdit,
    start,
    setDraft: (getDraft) => {
      draft = getDraft;
    },
  };
}
