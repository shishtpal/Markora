// Toggle task checkboxes by editing their original Markdown source.
export function connectLiveTasks(root, getDocument, change, canEdit = () => true) {
  root.addEventListener("change", (event) => {
    const box = event.target.closest('li input[type="checkbox"]');
    const node = box?.closest("[data-block]");
    if (!box || !node) return;
    if (!canEdit()) { box.checked = !box.checked; return; }
    const doc = getDocument();
    const block = doc.rendered.blocks[Number(node.dataset.block)];
    if (!block) return;
    const targetIndex = [...node.querySelectorAll('li input[type="checkbox"]')].indexOf(box);
    let seen = -1;
    let fence = false;
    const slice = doc.markdown.slice(block.start, block.end);
    const updated = slice
      .split("\n")
      .map((line) => {
        if (/^[ \t]*(?:```|~~~)/.test(line)) fence = !fence;
        if (fence) return line;
        return line.replace(/^([ \t]*(?:[-*+]|\d+[.)])\s+\[)([ xX])(\])/, (_, pre, _mark, post) => {
          seen += 1;
          return seen === targetIndex ? `${pre}${box.checked ? "x" : " "}${post}` : _;
        });
      })
      .join("\n");
    if (updated !== slice)
      change(
        doc.markdown.slice(0, block.start) + updated + doc.markdown.slice(block.end),
        true,
        true,
      );
  });
}
