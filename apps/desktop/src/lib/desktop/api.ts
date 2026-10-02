import { invoke } from '@tauri-apps/api/core';

export interface Collection { name: string; root: string; requests: string[]; folders: string[]; environments: string[]; readOnly: boolean; warning?: string | null }
export interface WorkspaceView { workspaces: { id: number; name: string }[]; activeWorkspaceId: number; collections: Collection[]; activeCollection: string | null; warning: string | null }
export interface FieldEdit { path: string[]; value: unknown }
export interface RequestInfo { name: string; method: string; url: string; revision: number; document: Record<string, any>; diagnostics: string[] }
export interface RequestSummary { name: string; method: string }
export interface ResponseMeta {
  id: number; status: number; headers: { name: string; value: string }[];
  bodyBytes: number; elapsedMs: number; text: boolean; historyWarning: string | null;
}
export interface SendInput { path: string; environment: string | null; method: string; url: string; baseUrl: string; revision?: number; edits: FieldEdit[]; secrets: Record<string, string>; document?: Record<string, any> }
export interface HistoryEntry { timestamp: number; requestPath: string; method: string; status: number | null; outcome: string; elapsedMs: number; bodyBytes: number }
export interface HistoryView { entries: HistoryEntry[]; warning: string | null }
export interface CreateRequestInput { name: string; fileName?: string; method?: string; url?: string; folder?: string; edits?: FieldEdit[] }
export interface RenameRequestInput { revision: number; name: string; fileName: string }
export interface RequestUpdate { collection: Collection; path: string; request: RequestInfo }

export const api = {
  workspace: () => invoke<WorkspaceView>('read_workspace'),
  createWorkspace: (name: string) => invoke<WorkspaceView>('create_workspace', { name }),
  renameWorkspace: (id: number, name: string) => invoke<WorkspaceView>('rename_workspace', { id, name }),
  removeWorkspace: (id: number) => invoke<WorkspaceView>('remove_workspace', { id }),
  selectWorkspace: (id: number) => invoke<WorkspaceView>('select_workspace', { id }),
  selectCollection: (root: string) => invoke<Collection>('select_collection', { root }),
  removeCollection: (root: string) => invoke<WorkspaceView>('remove_collection', { root }),
  open: () => invoke<Collection | null>('choose_collection'),
  example: () => invoke<Collection>('open_example'),
  createCollection: (name: string, folder: string) => invoke<Collection | null>('create_collection', { name, folder }),
  createRequest: (input: CreateRequestInput) => invoke<RequestUpdate>('create_request', { input }),
  createFolder: (parent: string, name: string, folder: string) => invoke<Collection>('create_folder', { parent, name, folder }),
  renameFolder: (folder: string, name: string) => invoke<Collection>('rename_folder', { folder, name }),
  renameRequest: (input: RenameRequestInput) => invoke<RequestUpdate>('rename_request', { input }),
  duplicateRequest: (input: RenameRequestInput) => invoke<RequestUpdate>('duplicate_request', { input }),
  request: (path: string) => invoke<RequestInfo>('read_request', { path }),
  requestSummaries: (root: string, paths: string[]) => invoke<Record<string, RequestSummary>>('read_request_summaries', { root, paths }),
  send: (input: SendInput) => invoke<ResponseMeta>('send_request', { input }),
  cancel: () => invoke<void>('cancel_request'),
  chunk: (id: number, offset: number) => invoke<string>('read_response', { id, offset }),
  save: (revision: number, edits: FieldEdit[]) => invoke<RequestInfo>('save_document', { input: { revision, edits } }),
  environment: (name: string) => invoke<RequestInfo>('read_environment', { name }),
  createEnvironment: (name: string) => invoke<RequestInfo>('create_environment', { name }),
  history: () => invoke<HistoryView>('read_history'),
  clearHistory: () => invoke<void>('clear_history'),
};
