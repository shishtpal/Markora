import { $, showError } from "./dom.js";

// One transport, a startup handshake, and a fresh draft on every outgoing action.
export function createBridge(receive) {
  const queue = [];
  let draft = () => null;
  let connected = false;
  let retryTimer;
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
    queue.push(JSON.stringify({ ...action, draft: draft() }));
    flush();
  }
  window.markoraReceive = (message) => {
    if (message.type === "state") {
      connected = true;
      clearInterval(retryTimer);
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
    retryTimer = setInterval(() => {
      flush();
      if (!connected && queue.length === 0) send({ type: "ready" });
    }, 200);
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
    start,
    setDraft: (getDraft) => {
      draft = getDraft;
    },
  };
}
