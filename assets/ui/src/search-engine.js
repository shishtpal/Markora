// Self-contained so the exact production engine also runs inside a cancellable worker.
export function searchDocument(request) {
  const {
    text,
    query,
    caseSensitive,
    wholeWord,
    regex,
    operation = "find",
    replacement = "",
    index = 0,
  } = request;
  const maximum = 100_000;
  if (!query) return { hits: [], limited: false };
  let pattern = regex ? query : query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  if (wholeWord) {
    const word = "[\\p{L}\\p{M}\\p{N}\\p{Pc}]";
    pattern = `(?<!${word})(?:${pattern})(?!${word})`;
  }
  const expression = new RegExp(pattern, `gmu${caseSensitive ? "" : "i"}`);
  const matches = [];
  let limited = false;
  for (const match of text.matchAll(expression)) {
    if (matches.length === maximum) {
      limited = true;
      break;
    }
    matches.push(match);
  }
  const hits = matches.map((match) => ({ start: match.index, length: match[0].length }));
  if (operation === "find" || !matches.length) return { hits, limited, replaced: 0 };
  if (limited && operation === "replaceAll")
    throw new Error("More than 100,000 matches. Refine the search before replacing all.");
  function expand(match) {
    if (!regex) return replacement;
    return replacement.replace(/\$(\$|&|`|'|<[^>]*>|\d{1,2})/g, (token, part) => {
      if (part === "$") return "$";
      if (part === "&") return match[0];
      if (part === "`") return text.slice(0, match.index);
      if (part === "'") return text.slice(match.index + match[0].length);
      if (part.startsWith("<"))
        return match.groups ? (match.groups[part.slice(1, -1)] ?? "") : token;
      const number = Number(part);
      if (number > 0 && number < match.length) return match[number] ?? "";
      const first = Number(part[0]);
      if (part.length === 2 && first > 0 && first < match.length)
        return (match[first] ?? "") + part[1];
      return token;
    });
  }
  const selected =
    operation === "replaceAll"
      ? matches
      : [matches[Math.max(0, Math.min(index, matches.length - 1))]];
  const parts = [];
  let cursor = 0,
    size = 0,
    anchor = 0;
  function append(part) {
    size += part.length;
    if (size > 32 * 1024 * 1024)
      throw new Error(
        "Replacement result exceeds 32 million characters. Refine the search or replacement.",
      );
    parts.push(part);
  }
  for (const match of selected) {
    const inserted = expand(match);
    append(text.slice(cursor, match.index));
    append(inserted);
    cursor = match.index + match[0].length;
    anchor = match.index + inserted.length;
    // An empty replacement at an empty match still advances by one Unicode code point.
    if (!match[0].length && !inserted.length)
      anchor += text.codePointAt(match.index) > 0xffff ? 2 : 1;
  }
  append(text.slice(cursor));
  return { hits, limited, text: parts.join(""), replaced: selected.length, anchor };
}
