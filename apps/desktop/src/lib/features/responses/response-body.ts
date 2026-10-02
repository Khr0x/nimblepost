// Bound formatting work; large responses remain available as their original text.
const MAX_PRETTY_LENGTH = 2 * 1024 * 1024;
export function formatJSON(body: string): { pretty: string | null; issue: 'invalid' | 'large' | null } {
  const source = body.replace(/^\uFEFF/, '').trim();
  if (source.length > MAX_PRETTY_LENGTH) return { pretty: null, issue: 'large' };
  try { JSON.parse(source); } catch { return { pretty: null, issue: 'invalid' }; }
  // Format tokens rather than stringify parsed values: preserve large integers,
  // number precision, duplicate keys and the server's original string escapes.
  let pretty = '', depth = 0, quoted = false, escaped = false, previous = '';
  const newline = () => '\n' + '  '.repeat(depth);
  for (let index = 0; index < source.length; index++) {
    const character = source[index];
    if (quoted) {
      pretty += character;
      if (escaped) escaped = false;
      else if (character === '\\') escaped = true;
      else if (character === '"') quoted = false;
    } else if (!/\s/.test(character)) {
      if (character === '"') { quoted = true; pretty += character; }
      else if (character === '{' || character === '[') {
        pretty += character; depth++;
        let next = index + 1;
        while (/\s/.test(source[next] ?? '') && next < source.length) next++;
        if (source[next] !== '}' && source[next] !== ']') pretty += newline();
      } else if (character === '}' || character === ']') {
        depth--;
        if (previous !== '{' && previous !== '[') pretty += newline();
        pretty += character;
      } else if (character === ',') pretty += ',' + newline();
      else if (character === ':') pretty += ': ';
      else pretty += character;
    }
    if (!/\s/.test(character) || quoted) previous = character;
    if (pretty.length > MAX_PRETTY_LENGTH) return { pretty: null, issue: 'large' };
  }
  return { pretty, issue: null };
}

export interface JSONToken {
  kind: 'key' | 'string' | 'number' | 'boolean' | 'null' | 'punctuation' | 'plain';
  text: string;
}
export function tokenizeJSON(pretty: string): JSONToken[] | null {
  const tokens: JSONToken[] = [];
  for (const match of pretty.matchAll(/"(?:\\.|[^"\\])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|[{}\[\]:,]|\s+/g)) {
    // ponytail: Bound DOM spans; use a virtualized editor for larger highlighted bodies.
    if (tokens.length >= 12000) return null;
    const text = match[0];
    let kind: JSONToken['kind'];
    if (text[0] === '"') {
      let next = match.index + text.length;
      while (/\s/.test(pretty[next] ?? '') && next < pretty.length) next++;
      kind = pretty[next] === ':' ? 'key' : 'string';
    } else if (text === 'true' || text === 'false') kind = 'boolean';
    else if (text === 'null') kind = 'null';
    else if (/^[\s]/.test(text)) kind = 'plain';
    else if (/^-?\d/.test(text)) kind = 'number';
    else kind = 'punctuation';
    tokens.push({ kind, text });
  }
  return tokens;
}

export interface JSONLine {
  id: number;
  tokens: JSONToken[];
  children?: JSONLine[];
  closing?: JSONToken[];
  label?: string;
}
export function foldJSON(tokens: JSONToken[]): JSONLine[] | null {
  const lines: JSONToken[][] = [[]];
  for (const token of tokens) {
    const parts = token.text.split('\n');
    for (let index = 0; index < parts.length; index++) {
      if (index) lines.push([]);
      if (parts[index]) lines.at(-1)!.push({ kind: token.kind, text: parts[index] });
    }
  }
  const root: JSONLine[] = [], stack: JSONLine[] = [];
  for (const [id, line] of lines.entries()) {
    const meaningful = line.filter(token => token.kind !== 'plain');
    const first = meaningful[0], last = meaningful.at(-1);
    if (first?.kind === 'punctuation' && (first.text === '}' || first.text === ']')) {
      const parent = stack.pop();
      if (!parent) return null;
      parent.closing = line;
    } else {
      const node: JSONLine = { id, tokens: line };
      (stack.at(-1)?.children ?? root).push(node);
      if (last?.kind === 'punctuation' && (last.text === '{' || last.text === '[')) {
        // ponytail: Bound recursive rendering depth; deeper documents keep the flat Pretty view.
        if (stack.length >= 128) return null;
        const key = meaningful.find(token => token.kind === 'key');
        node.label = `JSON ${last.text === '{' ? 'object' : 'array'}${key ? ` ${JSON.parse(key.text)}` : ''} at line ${id + 1}`;
        node.children = []; stack.push(node);
      }
    }
  }
  return stack.length ? null : root;
}
