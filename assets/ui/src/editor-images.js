// Selection offsets always refer to the Markdown document, including Live blocks.
export function createImageEdits(getDocument, isReading, live, source, change, flush) {
  function capture(target = null) {
    const doc = getDocument();
    if (isReading() || doc.kind === "pdf") return null;
    let start,
      end,
      block = false;
    if (doc.viewMode === "live") {
      const selection = live.selection();
      const node = target?.closest?.("[data-block]");
      if (selection && (!node || node.contains(live.textarea()))) ({ start, end } = selection);
      else {
        start = end = node
          ? (doc.rendered.blocks[Number(node.dataset.block)]?.end ?? doc.markdown.length)
          : doc.markdown.length;
        block = true;
      }
    } else {
      start = source.selectionStart;
      end = source.selectionEnd;
    }
    flush();
    return { docId: doc.id, revision: doc.revision, start, end, block };
  }
  function insert(anchor, images) {
    const doc = getDocument();
    if (
      !anchor ||
      isReading() ||
      doc.kind === "pdf" ||
      doc.id !== anchor.docId ||
      doc.revision !== anchor.revision
    )
      return false;
    const escaped = (alt) => alt.replace(/([\\\[\]])/g, "\\$1");
    const segment = (part) => encodeURIComponent(part).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
    // Encode each path segment once. Markdown destinations then contain no delimiters.
    const links = images
      .map(
        (image) =>
          `![${escaped(image.alt)}](${image.relativePath.startsWith("file:") ? image.relativePath : image.relativePath.split("/").map(segment).join("/")})`,
      )
      .join("\n");
    const before = doc.markdown.slice(0, anchor.start),
      after = doc.markdown.slice(anchor.end);
    const text =
      (anchor.block && before && !before.endsWith("\n\n") ? "\n\n" : "") +
      links +
      (anchor.block && after && !after.startsWith("\n\n") ? "\n\n" : "");
    live.reset();
    change(before + text + after, true, true);
    if (doc.viewMode !== "live") {
      source.setSelectionRange(anchor.start + text.length, anchor.start + text.length);
      source.focus();
    }
    return true;
  }
  return { capture, insert };
}
