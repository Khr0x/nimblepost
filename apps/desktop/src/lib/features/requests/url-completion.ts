export function isVariablePlaceholder(value: string) {
  return /^\{\{[ \t]*[\w.-]+[ \t]*\}\}$/.test(value);
}

export function urlHighlightParts(value: string, selectionStart: number, selectionEnd: number) {
  let offset = 0;
  return value.split(/(\{\{[ \t]*[\w.-]+[ \t]*\}\})/g).flatMap((text, index) => {
    const name = index % 2 === 1 ? text.slice(2, -2).trim() : undefined;
    const start = Math.max(0, Math.min(text.length, selectionStart - offset));
    const end = Math.max(start, Math.min(text.length, selectionEnd - offset));
    offset += text.length;
    return [
      { text: text.slice(0, start), name, variable: index % 2 === 1, selected: false },
      { text: text.slice(start, end), name, variable: index % 2 === 1, selected: true },
      { text: text.slice(end), name, variable: index % 2 === 1, selected: false },
    ].filter(part => part.text);
  });
}

export function urlVariableMatch(value: string, cursor: number) {
  const match = value.slice(0, cursor).match(/\{\{[ \t]*([\w.-]*)$/);
  return match ? { from: cursor - match[1].length, prefix: match[1] } : null;
}

export function completeUrlVariable(value: string, cursor: number, name: string) {
  const match = urlVariableMatch(value, cursor);
  if (!match) return null;
  const suffix = value.slice(cursor).match(/^[\w.-]*[ \t]*\}{0,2}/)?.[0] ?? '';
  const insert = `${name}}}`;
  return { value: value.slice(0, match.from) + insert + value.slice(cursor + suffix.length), cursor: match.from + insert.length };
}
