import type { RequestInfo, ResponseMeta } from '../../desktop/api';
import { draftFrom, type Draft } from './draft.ts';

export interface RequestTab {
  id: number;
  root: string | null;
  path: string;
  source: RequestInfo | null;
  draft: Draft;
  environment: string;
  baseUrl: string;
  response: ResponseMeta | null;
  body: string;
  offset: number;
}

export function untitledDraft(): Draft {
  return draftFrom({ name: 'Untitled', method: 'GET', url: '', revision: 0, diagnostics: [],
    document: { info: { name: 'Untitled', type: 'http' }, http: { method: 'GET', url: '' } } });
}

export function refreshTabDraft(tab: RequestTab, fresh: RequestInfo): Draft {
  if (!tab.draft.edits.length) return draftFrom(fresh);
  if (JSON.stringify(tab.source?.document) !== JSON.stringify(fresh.document)) {
    throw new Error('This request changed on disk. Save or Discard its pending changes before reopening it.');
  }
  return { ...tab.draft, revision: fresh.revision };
}
