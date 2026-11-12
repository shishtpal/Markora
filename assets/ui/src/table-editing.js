function rowCells(line, offset) {
  const pipes = [];
  for (let i = 0; i < line.length; i++) {
    if (line[i] !== "|") continue;
    let slashes = 0;
    for (let j = i - 1; j >= 0 && line[j] === "\\"; j--) slashes++;
    if (slashes % 2 === 0) pipes.push(i);
  }
  if (!pipes.length) return null;
  const boundaries = [-1, ...pipes, line.length];
  if (!line.slice(0, pipes[0]).trim()) boundaries.shift();
  if (!line.slice(pipes.at(-1) + 1).trim()) boundaries.pop();
  return boundaries.slice(0, -1).map((left, i) => {
    const right = boundaries[i + 1];
    const content = line.slice(left + 1, right);
    const leading = content.match(/^\s*/)[0].length;
    const start = offset + left + 1 + (content.trim() ? leading : Math.min(1, content.length));
    return {
      start,
      end: start + content.trim().length,
      text: content.trim(),
      left: offset + left,
      right: offset + right,
    };
  });
}
export function tableKeyEdit(text, start, end, key, shift) {
  if (!["Tab", "Enter"].includes(key) || (key === "Enter" && shift)) return null;
  const lines = text.split("\n");
  const rows = [];
  let offset = 0,
    current = 0;
  for (let i = 0; i < lines.length; i++) {
    rows.push({ offset, cells: rowCells(lines[i], offset) });
    if (offset <= start) current = i;
    offset += lines[i].length + 1;
  }
  if (!rows[current].cells?.length) return null;
  let first = current;
  while (first > 0 && rows[first - 1].cells?.length) first--;
  const columns = rows[first].cells.length;
  const separator = rows[first + 1]?.cells;
  if (
    !separator ||
    separator.length !== columns ||
    !separator.every((cell) => /^:?-+:?$/.test(cell.text)) ||
    current === first + 1
  )
    return null;
  let last = first + 1;
  while (rows[last + 1]?.cells?.length === columns) last++;
  if (current > last) return null;
  const cells = rows[current].cells;
  let column = cells.findIndex((cell) => start <= cell.right);
  if (column < 0) column = cells.length - 1;
  if (end > cells[column].right && start !== end) return null;
  let targetRow = current,
    targetColumn = column;
  if (key === "Tab") {
    targetColumn += shift ? -1 : 1;
    if (targetColumn < 0) {
      targetRow--;
      targetColumn = columns - 1;
    }
    if (targetColumn >= columns) {
      targetRow++;
      targetColumn = 0;
    }
    if (targetRow === first + 1) targetRow += shift ? -1 : 1;
  } else {
    targetRow++;
    if (targetRow === first + 1) targetRow++;
  }
  if (targetRow < first) return { value: text, start: cells[0].start, end: cells[0].end };
  if (targetRow <= last) {
    const cell = rows[targetRow].cells[targetColumn];
    return { value: text, start: cell.start, end: cell.end };
  }
  const indent = lines[current].match(/^[ \t]*/)[0];
  const row = indent + "|" + "   |".repeat(columns);
  const insertAt = rows[last].offset + lines[last].length;
  const cell = rowCells(row, insertAt + 1)[targetColumn];
  return {
    value: text.slice(0, insertAt) + "\n" + row + text.slice(insertAt),
    start: cell.start,
    end: cell.end,
  };
}
