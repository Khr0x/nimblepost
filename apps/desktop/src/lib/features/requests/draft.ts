import type { RequestInfo, FieldEdit } from '../../desktop/api';

// OpenCollection can contain fields this UI does not understand; keep them intact.
export type Config = Record<string, any>;
export interface Draft { revision: number; value: Config; edits: FieldEdit[]; diagnostics: string[] }
export function draftFrom(view: RequestInfo): Draft {
  return { revision: view.revision, value: JSON.parse(JSON.stringify(view.document)), edits: [], diagnostics: view.diagnostics };
}
export function editDraft(draft: Draft, path: string[], value: unknown) {
  let node = draft.value;
  for (let index = 0; index < path.length - 1; index++) {
    node[path[index]] ??= /^\d+$/.test(path[index + 1]) ? [] : {};
    node = node[path[index]];
  }
  const key = path.at(-1)!;
  if (value === null) { if (Array.isArray(node)) node.splice(Number(key), 1); else delete node[key]; }
  else node[key] = JSON.parse(JSON.stringify(value));
  const last = draft.edits.at(-1);
  // Coalesce typing in one field, preserving the order of row insertions/deletions.
  if (last && value !== null && last.value !== null && JSON.stringify(last.path) === JSON.stringify(path)) last.value = value;
  else draft.edits.push({ path, value });
}
