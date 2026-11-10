// Minimal DevTools client for the app's own embedded WebView2 regression tests.
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function connect(port) {
  let targets;
  for (let attempt = 0; attempt < 150; attempt++) {
    try {
      targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      if (targets.some((t) => t.type === "page")) break;
    } catch {}
    await delay(200);
  }
  const target = targets?.find((t) => t.type === "page");
  if (!target) throw new Error("WebView2 DevTools did not start within 30 seconds");
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = reject;
  });
  let nextId = 0;
  const pending = new Map();
  const errors = [];
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    if (message.method === "Runtime.exceptionThrown")
      errors.push(message.params.exceptionDetails.text);
    const request = pending.get(message.id);
    if (!request) return;
    pending.delete(message.id);
    clearTimeout(request.timer);
    if (message.error) request.reject(new Error(message.error.message));
    else request.resolve(message.result);
  };
  function call(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++nextId;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`DevTools timeout: ${method}`));
      }, 10000);
      pending.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method, params }));
    });
  }
  async function evaluate(expression) {
    const result = await call("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (result.exceptionDetails)
      throw new Error(
        result.exceptionDetails.exception?.description || result.exceptionDetails.text,
      );
    return result.result.value;
  }
  async function wait(expression, label = expression) {
    for (let attempt = 0; attempt < 100; attempt++) {
      if (await evaluate(expression)) return;
      await delay(50);
    }
    throw new Error(`UI condition failed: ${label}`);
  }
  await call("Runtime.enable");
  return { call, evaluate, wait, errors, close: () => socket.close() };
}
module.exports = { connect, delay };
