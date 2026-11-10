export const $ = (selector) => document.querySelector(selector);
export const $$ = (selector) => [...document.querySelectorAll(selector)];
export function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
export function showError(message) {
  $("#error-text").textContent = String(message);
  $("#error-banner").hidden = false;
}
export function resolveMedia(root, docId) {
  if (!docId || !root) return;
  const rewrite = (raw) => {
    if (!raw || /^(https?:|data:|blob:|#|\/)/i.test(raw)) return null;
    const clean = raw.replace(/\\/g, "/").replace(/^\.\//, "");
    const encoded = clean.split("/").filter(Boolean).map(encodeURIComponent).join("/");
    return encoded ? `http://markora.localhost/doc/${docId}/${encoded}` : null;
  };
  for (const el of root.querySelectorAll("audio, video, source, img")) {
    for (const attr of ["src", "poster"]) {
      const next = rewrite(el.getAttribute(attr));
      if (next) el.setAttribute(attr, next);
    }
  }
  for (const media of root.querySelectorAll("audio, video")) {
    if (media.querySelector("source[src]")) media.load();
  }
}
