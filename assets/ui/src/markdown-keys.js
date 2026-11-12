import { tableKeyEdit } from "./table-editing.js";

// Offsets and selections stay in textarea/JavaScript UTF-16 coordinates.
function lineStart(text, offset) {
  return offset === 0 ? 0 : text.lastIndexOf("\n", offset - 1) + 1;
}
export function inMarkdownFence(text, offset) {
  let fence = null;
  for (const line of text.slice(0, lineStart(text, offset)).split("\n")) {
    const match = line.match(/^\s*(?:>\s*)*(?:(?:[-+*]|\d{1,9}[.)])[ \t]+)?(`{3,}|~{3,})(.*)$/);
    if (!match) continue;
    if (!fence) {
      if (match[1][0] !== "`" || !match[2].includes("`")) fence = match[1];
    } else if (match[1][0] === fence[0] && match[1].length >= fence.length && !match[2].trim()) {
      fence = null;
    }
  }
  return fence !== null;
}
function indentation(text, start, end, outdent) {
  const first = lineStart(text, start);
  const last = lineStart(text, end > start && text[end - 1] === "\n" ? end - 1 : end);
  const edits = [];
  for (let pos = first; pos <= last;) {
    const removed = outdent ? text.slice(pos).match(/^(?:\t| {1,4})/)?.[0].length || 0 : 0;
    if (!outdent || removed) edits.push({ pos, removed, added: outdent ? "" : "    " });
    const next = text.indexOf("\n", pos);
    if (next < 0) break;
    pos = next + 1;
  }
  const map = (offset) =>
    edits.reduce(
      (result, edit) =>
        result +
        (edit.pos <= offset ? edit.added.length - Math.min(edit.removed, offset - edit.pos) : 0),
      offset,
    );
  let value = text;
  for (const edit of edits.slice().reverse()) {
    value = value.slice(0, edit.pos) + edit.added + value.slice(edit.pos + edit.removed);
  }
  return { value, start: map(start), end: map(end) };
}
export function markdownKeyEdit(text, start, end, key, shift = false, before = "") {
  const first = lineStart(text, start);
  const finish = text.indexOf("\n", end);
  const lineEnd = finish < 0 ? text.length : finish;
  const line = text.slice(first, lineEnd);
  const thematic = /^[ \t]*(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$/;
  const list = thematic.test(line.replace(/^[ \t]*(?:>[ \t]*)*/, ""))
    ? null
    : line.match(/^(\s*(?:>\s*)*)([-+*]|\d{1,9}[.)])([ \t]+)(\[[ xX]\](?:[ \t]+|$))?/);
  const fenced =
    (list || line.includes("|")) && inMarkdownFence(before + text, before.length + start);
  if (!fenced && line.includes("|") && lineStart(text, end) === first) {
    const table = tableKeyEdit(text, start, end, key, shift);
    if (table) return table;
  }
  if (key === "Tab") {
    if (shift || start !== end || (!fenced && list)) return indentation(text, start, end, shift);
    return {
      value: text.slice(0, start) + "    " + text.slice(end),
      start: start + 4,
      end: start + 4,
    };
  }
  if (key !== "Enter" || shift || lineStart(text, end) !== first) return null;
  const replace = (from, to, insert) => ({
    value: text.slice(0, from) + insert + text.slice(to),
    start: from + insert.length,
    end: from + insert.length,
  });
  if (!fenced && list && start >= first + list[0].length) {
    if (!line.slice(list[0].length).trim()) return replace(first, lineEnd, list[1]);
    const marker = /^\d/.test(list[2])
      ? `${Number.parseInt(list[2], 10) + 1}${list[2].at(-1)}`
      : list[2];
    return replace(start, end, `\n${list[1]}${marker}${list[3]}${list[4] ? "[ ] " : ""}`);
  }
  const indent = text.slice(first, start).match(/^[ \t]+/)?.[0];
  return indent ? replace(start, end, "\n" + indent) : null;
}
export function handleMarkdownKey(event, textarea, canEdit = () => true, before = "") {
  if (
    event.defaultPrevented ||
    event.isComposing ||
    event.keyCode === 229 ||
    event.ctrlKey ||
    event.metaKey ||
    event.altKey ||
    textarea.readOnly ||
    !canEdit() ||
    !["Tab", "Enter"].includes(event.key)
  )
    return false;
  const edit = markdownKeyEdit(
    textarea.value,
    textarea.selectionStart,
    textarea.selectionEnd,
    event.key,
    event.shiftKey,
    before,
  );
  if (!edit) return false;
  event.preventDefault();
  const direction = textarea.selectionDirection;
  if (edit.value !== textarea.value) {
    // Replace only the changed span, preserving the textarea's scroll position.
    let first = 0,
      oldEnd = textarea.value.length,
      newEnd = edit.value.length;
    while (first < oldEnd && first < newEnd && textarea.value[first] === edit.value[first]) first++;
    while (
      oldEnd > first &&
      newEnd > first &&
      textarea.value[oldEnd - 1] === edit.value[newEnd - 1]
    ) {
      oldEnd--;
      newEnd--;
    }
    textarea.setRangeText(edit.value.slice(first, newEnd), first, oldEnd, "preserve");
    textarea.setSelectionRange(edit.start, edit.end, direction);
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
  } else textarea.setSelectionRange(edit.start, edit.end, direction);
  return true;
}
