import type { RequestInfo, RequestInspection, ResponseMeta } from '../../desktop/api';
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
  restored?: boolean;
  recoveryNotice?: string;
  externalChange?: string;
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
  return { ...tab.draft, revision: fresh.revision, diagnostics: fresh.diagnostics };
}

export function refreshExternalTab(tab: RequestTab, inspection: RequestInspection): boolean {
  const changed = inspection.changed === true || JSON.stringify(tab.source?.document) !== JSON.stringify(inspection.request.document);
  if (!changed) { tab.externalChange = ''; return false; }
  if (tab.draft.edits.length) {
    tab.externalChange = 'This file changed outside NimblePost. Your draft is preserved. Reload from disk to discard it, or Save copy to keep both versions.';
    return false;
  }
  tab.source = inspection.request; tab.draft = draftFrom(inspection.request);
  tab.externalChange = ''; tab.restored = false;
  tab.response = null; tab.body = ''; tab.offset = 0;
  return true;
}
