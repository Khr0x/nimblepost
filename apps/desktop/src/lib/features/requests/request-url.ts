import { editDraft, type Config, type Draft } from './draft.ts';

function splitUrl(value: string) {
  const hash = value.indexOf('#');
  const address = hash < 0 ? value : value.slice(0, hash);
  const query = address.indexOf('?');
  return { base: query < 0 ? address : address.slice(0, query),
    query: query < 0 ? '' : address.slice(query + 1), fragment: hash < 0 ? '' : value.slice(hash) };
}

function encodeParam(value: string) {
  // Keep placeholders editable and visible; encode the surrounding literal text.
  return value.split(/(\{\{[ \t]*[\w.-]+[ \t]*\}\})/g).map((part, index) =>
    index % 2 ? part : new URLSearchParams([['', part]]).toString().slice(1)).join('');
}

export function requestUrlWithParams(url: string, params: Config[] = []) {
  const entries = params.filter(row => !row.disabled && row.type === 'query').map(row => {
    const value = typeof row.value === 'string' ? row.value : row.value?.type === 'string' ? row.value.data : '';
    return `${encodeParam(row.name ?? '')}=${encodeParam(value ?? '')}`;
  });
  if (!entries.length) return url;
  const { base, query, fragment } = splitUrl(url);
  return `${base}?${[query, ...entries].filter(Boolean).join('&')}${fragment}`;
}

export function editRequestUrl(draft: Draft, text: string) {
  const { base, query, fragment } = splitUrl(text);
  const rows: Config[] = Array.isArray(draft.value.http?.params) ? draft.value.http.params : [];
  const available = rows.filter(row => !row.disabled && row.type === 'query');
  const parsed: Config[] = [];
  new URLSearchParams(query).forEach((value, name) => {
    const index = available.findIndex(row => row.name === name);
    const original = index < 0 ? {} : available.splice(index, 1)[0];
    parsed.push({ ...original, type: 'query', name, value: original.value?.type === 'string'
      ? { ...original.value, data: value } : value });
  });
  // Keep disabled and unsupported rows, including their metadata and relative order.
  const pending = [...parsed];
  const params = rows.flatMap(row => row.disabled || row.type !== 'query' ? [row] : pending.length ? [pending.shift()!] : []);
  params.push(...pending);
  const url = base + fragment;
  if (draft.value.http?.url !== url) editDraft(draft, ['http', 'url'], url);
  if (JSON.stringify(rows) !== JSON.stringify(params)) editDraft(draft, ['http', 'params'], params);
}
