import type { Collection, FieldEdit, RequestInfo } from '../../desktop/api';
import type { Config, Draft } from './draft';
import type { RequestTab } from './request-tabs';

export interface RecoveryTab {
  id: number; root: string | null; path: string; environment: string;
  value: Config; edits: FieldEdit[]; source: Config | null;
}
export interface RecoverySnapshot {
  version: 1; workspaceId: number; activeTab: number | null; home: boolean; tabs: RecoveryTab[];
}
export interface RecoveryView { snapshot: RecoverySnapshot | null; warning: string | null }

export function captureRecovery(workspaceId: number, tabs: RequestTab[], activeId: number, draft: Draft | null, environment: string, home: boolean): RecoverySnapshot {
  return JSON.parse(JSON.stringify({ version: 1, workspaceId, activeTab: tabs.some(tab => tab.id === activeId) ? activeId : tabs.at(-1)?.id ?? null, home,
    tabs: tabs.map(tab => {
      const current = tab.id === activeId;
      const value = current && draft ? draft : tab.draft;
      return { id: tab.id, root: tab.root, path: tab.path, environment: current ? environment : tab.environment,
        value: value.value, edits: value.edits, source: tab.path && value.edits.length ? tab.source?.document ?? null : null };
    }) }));
}

export function restoreRecovery(snapshot: RecoverySnapshot, collections: Collection[]): RequestTab[] {
  return snapshot.tabs.map(saved => {
    const owner = collections.find(owner => owner.root === saved.root && !owner.warning);
    const missing = !!saved.path && (!owner || !owner.requests.includes(saved.path));
    const source: RequestInfo | null = saved.source ? { name: saved.source.info?.name ?? 'Untitled', method: saved.source.http?.method ?? 'GET', url: saved.source.http?.url ?? '', revision: 0, diagnostics: [], document: saved.source } : null;
    const tab: RequestTab = { id: saved.id, root: owner?.root ?? null, path: saved.path, source,
      draft: { revision: 0, value: saved.value, edits: saved.edits, diagnostics: [] }, environment: owner?.environments.includes(saved.environment) ? saved.environment : '',
      baseUrl: '', response: null, body: '', offset: 0, restored: true };
    if (missing) recoverAsUntitled(tab, 'The original file is unavailable.');
    return tab;
  });
}

export function recoverAsUntitled(tab: RequestTab, reason: string) {
  tab.path = ''; tab.source = null; tab.draft.revision = 0; tab.restored = false; tab.externalChange = '';
  if (!tab.draft.edits.length) tab.draft.edits.push({ path: ['info', 'name'], value: tab.draft.value.info?.name ?? 'Untitled' });
  tab.recoveryNotice = `${reason} Your request was recovered as an unsaved copy; Save to choose a location.`;
}
