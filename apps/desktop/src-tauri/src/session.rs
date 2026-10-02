use crate::workspaces::{CollectionRef, Registry, Store};
use base64::Engine;
use nimblepost_core::{
    CancellationToken, CollectionIndex, Document, DocumentKind, ExecutionContext, FieldEdit,
    History, HistoryEntry, HttpResponse, LoadedRequest, collection_index, create_collection,
    create_environment, create_folder, create_request_with_edits, duplicate_request, execute,
    load_environment, load_request, prepare, read_within, rename_folder, rename_request,
    save_document,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

const CHUNK_BYTES: usize = 64 * 1024;

#[derive(Default)]
pub struct Session {
    inner: Mutex<Inner>,
    operation: tokio::sync::Mutex<()>,
    history: Arc<Mutex<Option<History>>>,
    history_warning: Mutex<Option<String>>,
    workspaces: Arc<Mutex<Store>>,
    workspace_warning: Mutex<Option<String>>,
}

#[derive(Default)]
struct Inner {
    root: Option<PathBuf>,
    read_only: bool,
    next_id: u64,
    active: Option<(u64, CancellationToken)>,
    response: Option<(u64, HttpResponse)>,
    snapshots: BTreeMap<u64, (DocumentKind, Document)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionView {
    name: String,
    root: String,
    requests: Vec<String>,
    folders: Vec<String>,
    environments: Vec<String>,
    read_only: bool,
    warning: Option<String>,
}

#[derive(Serialize)]
pub struct WorkspaceSummary {
    id: u32,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    workspaces: Vec<WorkspaceSummary>,
    active_workspace_id: u32,
    collections: Vec<CollectionView>,
    active_collection: Option<String>,
    warning: Option<String>,
}

#[derive(Serialize)]
pub struct RequestView {
    name: String,
    method: String,
    url: String,
    revision: u64,
    document: serde_json::Value,
    diagnostics: Vec<String>,
}

#[derive(Serialize)]
pub struct RequestSummary {
    name: String,
    method: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendInput {
    pub path: String,
    pub environment: Option<String>,
    pub method: String,
    pub url: String,
    pub base_url: String,
    #[serde(default)]
    pub revision: Option<u64>,
    #[serde(default)]
    pub edits: Vec<FieldEdit>,
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
    #[serde(default)]
    pub document: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveInput {
    pub revision: u64,
    pub edits: Vec<FieldEdit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRequestInput {
    pub name: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default = "default_request_method")]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub edits: Vec<FieldEdit>,
}

fn default_request_method() -> String {
    "GET".into()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameRequestInput {
    pub revision: u64,
    pub name: String,
    pub file_name: String,
}

#[derive(Serialize)]
pub struct RequestUpdate {
    collection: CollectionView,
    path: String,
    request: RequestView,
}

impl CollectionView {
    fn from_index(index: CollectionIndex, read_only: bool) -> Self {
        Self {
            name: index.name,
            root: index.root.to_string_lossy().into(),
            requests: index.requests,
            folders: index.folders,
            environments: index.environments,
            read_only,
            warning: None,
        }
    }
}

#[derive(Serialize)]
pub struct HistoryView {
    entries: Vec<HistoryEntry>,
    warning: Option<String>,
}

#[derive(Serialize)]
pub struct HeaderView {
    name: String,
    value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMeta {
    id: u64,
    status: u16,
    headers: Vec<HeaderView>,
    body_bytes: usize,
    elapsed_ms: u128,
    text: bool,
    history_warning: Option<String>,
}

struct ActiveGuard<'a> {
    session: &'a Session,
    id: u64,
}
impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        let mut inner = self.session.inner.lock().unwrap();
        if inner.active.as_ref().is_some_and(|(id, _)| *id == self.id) {
            inner.active = None;
        }
    }
}

impl Session {
    pub fn init_workspaces(&self, path: PathBuf) {
        match Store::open(path) {
            Ok(store) => *self.workspaces.lock().unwrap() = store,
            Err(error) => {
                *self.workspace_warning.lock().unwrap() =
                    Some(format!("Workspaces could not be loaded: {error}"))
            }
        }
    }
    fn ensure_idle(&self) -> Result<(), String> {
        if self.inner.lock().unwrap().active.is_some() {
            return Err(
                "Cancel the active request before switching collections or workspaces".into(),
            );
        }
        Ok(())
    }
    fn clear_collection(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.root = None;
        inner.read_only = false;
        inner.response = None;
        inner.snapshots.clear();
    }
    async fn edit_workspaces(
        &self,
        edit: impl FnOnce(&mut Registry) -> Result<(), String> + Send + 'static,
    ) -> Result<Registry, String> {
        if let Some(warning) = self.workspace_warning.lock().unwrap().clone() {
            return Err(warning);
        }
        let store = self.workspaces.clone();
        tokio::task::spawn_blocking(move || store.lock().unwrap().update(edit))
            .await
            .map_err(|_| "Could not save workspace references")?
    }
    async fn workspace_view(&self, registry: Registry, restore: bool) -> WorkspaceView {
        let workspace = registry.active();
        let mut collections = vec![];
        // ponytail: sequential lazy indexing per collection; bounded parallel indexing if startup profiles require it.
        for reference in &workspace.collections {
            let index = collection_index(&reference.root).await.map_err(|error| error.to_string())
                .and_then(|index| if index.root == reference.root { Ok(index) } else {
                    Err("Collection folder now points to a different location. Remove its reference and open it again".into())
                });
            let view = match index {
                Ok(index) => CollectionView::from_index(index, reference.read_only),
                Err(error) => CollectionView {
                    name: reference.name.clone(),
                    root: reference.root.to_string_lossy().into(),
                    read_only: reference.read_only,
                    requests: vec![],
                    folders: vec![],
                    environments: vec![],
                    warning: Some(error.to_string()),
                },
            };
            collections.push(view);
        }
        if restore {
            let selected = workspace.active_collection.as_ref().and_then(|root| {
                collections.iter().find(|collection| {
                    Path::new(&collection.root) == root && collection.warning.is_none()
                })
            });
            let mut inner = self.inner.lock().unwrap();
            let root = selected.map(|collection| PathBuf::from(&collection.root));
            if inner.root != root {
                inner.response = None;
                inner.snapshots.clear();
            }
            inner.root = root;
            inner.read_only = selected.is_some_and(|collection| collection.read_only);
        }
        let active_collection = self
            .inner
            .lock()
            .unwrap()
            .root
            .as_ref()
            .map(|root| root.to_string_lossy().into());
        WorkspaceView {
            active_workspace_id: registry.active_workspace,
            workspaces: registry
                .workspaces
                .iter()
                .map(|workspace| WorkspaceSummary {
                    id: workspace.id,
                    name: workspace.name.clone(),
                })
                .collect(),
            collections,
            active_collection,
            warning: self.workspace_warning.lock().unwrap().clone(),
        }
    }
    pub async fn read_workspace(&self) -> Result<WorkspaceView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        self.ensure_idle()?;
        let registry = self.workspaces.lock().unwrap().registry.clone();
        Ok(self.workspace_view(registry, true).await)
    }
    pub async fn create_workspace(&self, name: String) -> Result<WorkspaceView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        self.ensure_idle()?;
        let registry = self
            .edit_workspaces(move |registry| registry.create(name))
            .await?;
        self.clear_collection();
        Ok(self.workspace_view(registry, true).await)
    }
    pub async fn select_workspace(&self, id: u32) -> Result<WorkspaceView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        self.ensure_idle()?;
        let registry = self
            .edit_workspaces(move |registry| {
                if !registry
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.id == id)
                {
                    return Err("Workspace not found".into());
                }
                registry.active_workspace = id;
                Ok(())
            })
            .await?;
        self.clear_collection();
        Ok(self.workspace_view(registry, true).await)
    }
    pub async fn rename_workspace(&self, id: u32, name: String) -> Result<WorkspaceView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        self.ensure_idle()?;
        let registry = self
            .edit_workspaces(move |registry| registry.rename(id, name))
            .await?;
        Ok(self.workspace_view(registry, true).await)
    }
    pub async fn remove_workspace(&self, id: u32) -> Result<WorkspaceView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        self.ensure_idle()?;
        let was_active = self.workspaces.lock().unwrap().registry.active_workspace == id;
        let registry = self
            .edit_workspaces(move |registry| registry.remove(id))
            .await?;
        if was_active {
            self.clear_collection();
        }
        Ok(self.workspace_view(registry, true).await)
    }
    pub async fn select_collection(&self, root: PathBuf) -> Result<CollectionView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        self.ensure_idle()?;
        let reference = self
            .workspaces
            .lock()
            .unwrap()
            .registry
            .active()
            .collections
            .iter()
            .find(|collection| collection.root == root)
            .cloned()
            .ok_or("Collection is not in the active workspace")?;
        let index = collection_index(root)
            .await
            .map_err(|error| error.to_string())?;
        if index.root != reference.root {
            return Err("Collection folder now points to a different location. Remove its reference and open it again".into());
        }
        self.adopt_collection(index, reference.read_only).await
    }
    pub async fn remove_collection(&self, root: PathBuf) -> Result<WorkspaceView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        self.ensure_idle()?;
        let registry = self
            .edit_workspaces(move |registry| {
                let workspace = registry.active_mut();
                if !workspace
                    .collections
                    .iter()
                    .any(|collection| collection.root == root)
                {
                    return Err("Collection is not in the active workspace".into());
                }
                workspace
                    .collections
                    .retain(|collection| collection.root != root);
                if workspace.active_collection.as_ref() == Some(&root) {
                    workspace.active_collection = workspace
                        .collections
                        .first()
                        .map(|collection| collection.root.clone());
                }
                Ok(())
            })
            .await?;
        Ok(self.workspace_view(registry, true).await)
    }

    pub async fn open(&self, root: PathBuf) -> Result<CollectionView, String> {
        self.open_with_access(root, false).await
    }
    pub async fn open_example(&self, root: PathBuf) -> Result<CollectionView, String> {
        self.open_with_access(root, true).await
    }
    async fn open_with_access(
        &self,
        root: PathBuf,
        read_only: bool,
    ) -> Result<CollectionView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let index = collection_index(root).await.map_err(|e| e.to_string())?;
        self.adopt_collection(index, read_only).await
    }

    async fn adopt_collection(
        &self,
        index: CollectionIndex,
        read_only: bool,
    ) -> Result<CollectionView, String> {
        self.ensure_idle()?;
        let reference = CollectionRef {
            root: index.root.clone(),
            name: index.name.clone(),
            read_only,
        };
        self.edit_workspaces(move |registry| {
            registry.attach(reference);
            Ok(())
        })
        .await?;
        let mut inner = self.inner.lock().unwrap();
        inner.root = Some(index.root.clone());
        inner.read_only = read_only;
        inner.response = None;
        inner.snapshots.clear();
        Ok(CollectionView::from_index(index, read_only))
    }

    pub async fn create_collection(
        &self,
        parent: PathBuf,
        name: String,
        folder: String,
    ) -> Result<CollectionView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        if self.inner.lock().unwrap().active.is_some() {
            return Err("Cancel the active request before creating a collection".into());
        }
        if let Some(warning) = self.workspace_warning.lock().unwrap().clone() {
            return Err(warning);
        }
        let root = create_collection(parent, name, folder)
            .await
            .map_err(|e| e.to_string())?;
        let index = collection_index(root).await.map_err(|e| e.to_string())?;
        let created_path = index.root.display().to_string();
        self.adopt_collection(index, false).await.map_err(|error| format!("Collection created at {created_path}, but could not be added to the workspace: {error}"))
    }

    pub async fn create_request(&self, input: CreateRequestInput) -> Result<RequestUpdate, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let root = self.writable_root()?;
        let index = collection_index(&root).await.map_err(|e| e.to_string())?;
        if index.requests.len() >= 10_000 {
            return Err("This version indexes up to 10,000 requests".into());
        }
        let document = create_request_with_edits(
            root,
            input.folder,
            input.name,
            input.file_name,
            input.method,
            input.url,
            input.edits,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(self.request_update(index, document, None))
    }

    fn writable_root(&self) -> Result<PathBuf, String> {
        let inner = self.inner.lock().unwrap();
        if inner.read_only {
            return Err(
                "Included examples are read-only. Create or open a collection to make changes"
                    .into(),
            );
        }
        if inner.active.is_some() {
            return Err("Cancel the active request before changing the collection".into());
        }
        inner.root.clone().ok_or("Open a collection first".into())
    }

    fn request_update(
        &self,
        mut index: CollectionIndex,
        document: Document,
        replaced: Option<&Path>,
    ) -> RequestUpdate {
        let path = document
            .path()
            .strip_prefix(&index.root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(replaced) = replaced {
            let old = replaced
                .strip_prefix(&index.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            index.requests.retain(|path| *path != old);
        }
        if !index.requests.contains(&path) {
            index.requests.push(path.clone());
        }
        index.requests.sort();
        RequestUpdate {
            collection: CollectionView::from_index(index, false),
            path,
            request: self.snapshot(DocumentKind::HttpRequest, document),
        }
    }

    pub async fn create_folder(
        &self,
        parent: String,
        name: String,
        folder: String,
    ) -> Result<CollectionView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let root = self.writable_root()?;
        let mut index = collection_index(&root).await.map_err(|e| e.to_string())?;
        if index.folders.len() >= 10_000 {
            return Err("This version indexes up to 10,000 folders".into());
        }
        let path = create_folder(root, parent, name, folder)
            .await
            .map_err(|e| e.to_string())?;
        index.folders.push(
            path.strip_prefix(&index.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
        );
        index.folders.sort();
        Ok(CollectionView::from_index(index, false))
    }

    pub async fn rename_folder(
        &self,
        folder: String,
        name: String,
    ) -> Result<CollectionView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let root = self.writable_root()?;
        let mut index = collection_index(&root).await.map_err(|e| e.to_string())?;
        if !index.folders.contains(&folder) {
            return Err("Select an indexed collection folder".into());
        }
        let renamed = rename_folder(root.clone(), folder.clone(), name)
            .await
            .map_err(|e| e.to_string())?;
        let destination = renamed
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let prefix = format!("{folder}/");
        for path in index.folders.iter_mut().chain(index.requests.iter_mut()) {
            if *path == folder {
                *path = destination.clone();
            } else if let Some(tail) = path.strip_prefix(&prefix) {
                *path = format!("{destination}/{tail}");
            }
        }
        index.folders.sort();
        index.requests.sort();
        self.inner
            .lock()
            .unwrap()
            .snapshots
            .retain(|_, (_, doc)| !doc.path().starts_with(root.join(&folder)));
        Ok(CollectionView::from_index(index, false))
    }

    pub async fn rename_request(&self, input: RenameRequestInput) -> Result<RequestUpdate, String> {
        self.copy_request(input, true).await
    }

    pub async fn duplicate_request(
        &self,
        input: RenameRequestInput,
    ) -> Result<RequestUpdate, String> {
        self.copy_request(input, false).await
    }

    async fn copy_request(
        &self,
        input: RenameRequestInput,
        rename: bool,
    ) -> Result<RequestUpdate, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let root = self.writable_root()?;
        let index = collection_index(&root).await.map_err(|e| e.to_string())?;
        if !rename && index.requests.len() >= 10_000 {
            return Err("This version indexes up to 10,000 requests".into());
        }
        let (kind, original, _) = self.draft(input.revision, &[])?;
        if kind != DocumentKind::HttpRequest {
            return Err("Select an HTTP request".into());
        }
        let old = original.path().to_owned();
        let document = if rename {
            rename_request(root, original, input.name, input.file_name).await
        } else {
            duplicate_request(root, original, input.name, input.file_name).await
        }
        .map_err(|e| e.to_string())?;
        if rename {
            self.inner.lock().unwrap().snapshots.remove(&input.revision);
        }
        Ok(self.request_update(index, document, rename.then_some(old.as_path())))
    }

    fn root(&self) -> Result<PathBuf, String> {
        self.inner
            .lock()
            .unwrap()
            .root
            .clone()
            .ok_or_else(|| "Open a collection first".into())
    }

    pub async fn request_summaries(
        &self,
        root: PathBuf,
        paths: Vec<String>,
    ) -> Result<BTreeMap<String, RequestSummary>, String> {
        if paths.len() > 128 {
            return Err("Read at most 128 request summaries at a time".into());
        }
        if !self
            .workspaces
            .lock()
            .unwrap()
            .registry
            .active()
            .collections
            .iter()
            .any(|collection| collection.root == root)
        {
            return Err("Collection is not in the active workspace".into());
        }
        if tokio::fs::canonicalize(&root)
            .await
            .map_err(|error| error.to_string())?
            != root
        {
            return Err("Collection folder now points to a different location".into());
        }
        let mut summaries = BTreeMap::new();
        for path in paths {
            let Ok(document) = read_within(&root, Path::new(&path)).await else {
                continue;
            };
            let value = document.value();
            if value["info"]["type"] != "http" {
                continue;
            }
            let Some(method) = value["http"]["method"]
                .as_str()
                .filter(|method| !method.is_empty())
            else {
                continue;
            };
            summaries.insert(
                path.clone(),
                RequestSummary {
                    name: value["info"]["name"]
                        .as_str()
                        .filter(|name| !name.trim().is_empty())
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            Path::new(&path)
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into()
                        }),
                    method: method.to_ascii_uppercase(),
                },
            );
        }
        Ok(summaries)
    }

    pub async fn read_request(&self, path: &str) -> Result<RequestView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let document = read_within(&self.root()?, Path::new(path))
            .await
            .map_err(|e| e.to_string())?;
        if document.value()["info"]["type"] != "http" {
            return Err("Select an HTTP request".into());
        }
        Ok(self.snapshot(DocumentKind::HttpRequest, document))
    }

    fn snapshot(&self, kind: DocumentKind, document: Document) -> RequestView {
        let mut inner = self.inner.lock().unwrap();
        inner.next_id += 1;
        let revision = inner.next_id;
        inner
            .snapshots
            .retain(|_, (_, old)| old.path() != document.path());
        let value = document.value().clone();
        let view = RequestView {
            revision,
            name: value["info"]["name"]
                .as_str()
                .or(value["name"].as_str())
                .unwrap_or("Untitled")
                .into(),
            method: value["http"]["method"].as_str().unwrap_or("GET").into(),
            url: value["http"]["url"].as_str().unwrap_or("").into(),
            diagnostics: document.diagnostics(kind),
            document: value,
        };
        inner.snapshots.insert(revision, (kind, document));
        while inner.snapshots.len() > 8 {
            inner.snapshots.pop_first();
        }
        view
    }

    fn draft(
        &self,
        revision: u64,
        edits: &[FieldEdit],
    ) -> Result<(DocumentKind, Document, Document), String> {
        let (kind, original) = self
            .inner
            .lock()
            .unwrap()
            .snapshots
            .get(&revision)
            .cloned()
            .ok_or("This document revision expired. Reload it")?;
        let root = self.root()?;
        if !original.path().starts_with(&root) {
            return Err("The document belongs to another collection".into());
        }
        let edited = original.edited(kind, edits).map_err(|e| e.to_string())?;
        Ok((kind, original, edited))
    }

    pub async fn read_environment(&self, name: &str) -> Result<RequestView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let document = load_environment(&self.root()?, name)
            .await
            .map_err(|e| e.to_string())?;
        Ok(self.snapshot(DocumentKind::Environment, document))
    }
    pub async fn create_environment(&self, name: String) -> Result<RequestView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let root = self.root()?;
        if self.inner.lock().unwrap().read_only {
            return Err(
                "Included examples are read-only. Open a collection folder to save changes".into(),
            );
        }
        if collection_index(&root)
            .await
            .map_err(|e| e.to_string())?
            .environments
            .contains(&name)
        {
            return Err("Environment name already exists".into());
        }
        let document = create_environment(root, name)
            .await
            .map_err(|e| e.to_string())?;
        Ok(self.snapshot(DocumentKind::Environment, document))
    }
    pub async fn save(&self, input: SaveInput) -> Result<RequestView, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        if self.inner.lock().unwrap().read_only {
            return Err(
                "Included examples are read-only. Open a collection folder to save changes".into(),
            );
        }
        let (kind, original, edited) = self.draft(input.revision, &input.edits)?;
        if kind == DocumentKind::Environment && original.value()["name"] != edited.value()["name"] {
            let name = edited.value()["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or("Environment name cannot be empty")?;
            if collection_index(self.root()?)
                .await
                .map_err(|e| e.to_string())?
                .environments
                .iter()
                .any(|existing| existing == name)
            {
                return Err("Environment name already exists".into());
            }
        }
        let saved = save_document(original, edited)
            .await
            .map_err(|e| e.to_string())?;
        self.inner.lock().unwrap().snapshots.remove(&input.revision);
        Ok(self.snapshot(kind, saved))
    }

    pub fn init_history(&self, path: PathBuf) {
        match History::open(path) {
            Ok(history) => *self.history.lock().unwrap() = Some(history),
            Err(error) => *self.history_warning.lock().unwrap() = Some(error.to_string()),
        }
    }
    pub fn history(&self) -> HistoryView {
        HistoryView {
            entries: self
                .history
                .lock()
                .unwrap()
                .as_ref()
                .map(|history| history.entries().to_vec())
                .unwrap_or_default(),
            warning: self.history_warning.lock().unwrap().clone(),
        }
    }
    pub async fn clear_history(&self) -> Result<(), String> {
        let history = self.history.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(history) = &mut *history.lock().unwrap() {
                history.clear().map_err(|e| e.to_string())?;
            }
            Ok(())
        })
        .await
        .map_err(|_| "Could not clear history")?
    }
    async fn record_history(&self, entry: HistoryEntry) -> Option<String> {
        let history = self.history.clone();
        let result = tokio::task::spawn_blocking(move || {
            if let Some(history) = &mut *history.lock().unwrap() {
                history.record(entry).map_err(|e| e.to_string())?;
                return Ok::<_, String>(true);
            }
            Ok(false)
        })
        .await;
        let warning = match result {
            Ok(Ok(true)) => None,
            Ok(Ok(false)) => self.history_warning.lock().unwrap().clone(),
            Ok(Err(error)) => Some(error),
            Err(_) => Some("Could not write history".into()),
        };
        *self.history_warning.lock().unwrap() = warning.clone();
        warning
    }

    pub async fn send(&self, input: SendInput) -> Result<ResponseMeta, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another operation is running")?;
        let token = CancellationToken::new();
        let (root, id) = {
            let mut inner = self.inner.lock().unwrap();
            let root = inner.root.clone();
            if root.is_none() && input.document.is_none() {
                return Err("Open a collection first".into());
            }
            if inner.active.is_some() {
                return Err("A request is already running".into());
            }
            inner.next_id += 1;
            let id = inner.next_id;
            inner.active = Some((id, token.clone()));
            inner.response = None;
            (root, id)
        };
        let _guard = ActiveGuard { session: self, id };
        let started = Instant::now();
        let history_path = input.path.clone();
        let history_method = input.method.clone();
        let result = tokio::select! {
            biased;
            _ = token.cancelled() => Err(nimblepost_core::Error::Cancelled),
            result = async {
                let transient = input.document.is_some();
                let mut loaded = if let Some(value) = input.document {
                    let path = root.as_ref().map(|root|root.join("Untitled.yml")).unwrap_or_else(||PathBuf::from("Untitled.yml"));
                    let document = Document::from_yaml(path, value.to_string())?;
                    match &root {
                        Some(root) => LoadedRequest::in_collection(root, document, input.environment.as_deref()).await?,
                        None => LoadedRequest::standalone(document)?,
                    }
                } else {
                    load_request(root.unwrap(), &input.path, input.environment.as_deref()).await?
                };
                if let Some(revision) = input.revision.filter(|_|!transient) {
                    let (_, original, edited) = self.draft(revision, &input.edits).map_err(|_| nimblepost_core::Error::Config { path: loaded.request.path().into(), field: "draft".into(), message: "El borrador no admite ejecución; guarda o recarga" })?;
                    if loaded.request.path() != original.path() || loaded.request.original() != original.original() { return Err(nimblepost_core::Error::Conflict { path: original.path().into(), temporary: None }); }
                    loaded.request = edited;
                }
                loaded.set_http_target(input.method, input.url);
                let context = ExecutionContext { overrides: if input.base_url.is_empty() { BTreeMap::new() }
                    else { BTreeMap::from([("baseUrl".into(), input.base_url)]) }, secrets: input.secrets };
                execute(prepare(&loaded, &context)?, &token).await
            } => result,
        };
        let outcome = match &result {
            Ok(_) => "response",
            Err(nimblepost_core::Error::Cancelled) => "cancelled",
            Err(nimblepost_core::Error::Timeout { .. }) => "timeout",
            Err(
                nimblepost_core::Error::Network(_) | nimblepost_core::Error::BodyTooLarge { .. },
            ) => "transport",
            Err(_) => "configuration",
        };
        let elapsed_ms = started.elapsed().as_millis();
        let history_warning = self
            .record_history(HistoryEntry::new(
                &history_path,
                &history_method,
                result.as_ref().ok().map(|r| r.status),
                outcome,
                elapsed_ms as u64,
                result
                    .as_ref()
                    .ok()
                    .map(|r| r.body.len())
                    .unwrap_or_default(),
            ))
            .await;
        let response = result.map_err(|e| e.to_string())?;
        let meta = ResponseMeta {
            id,
            status: response.status,
            body_bytes: response.body.len(),
            elapsed_ms,
            text: std::str::from_utf8(&response.body).is_ok(),
            history_warning,
            headers: response
                .headers
                .iter()
                .map(|header| HeaderView {
                    name: header.name.clone(),
                    value: String::from_utf8_lossy(&header.value).into(),
                })
                .collect(),
        };
        self.inner.lock().unwrap().response = Some((id, response));
        Ok(meta)
    }

    pub fn cancel(&self) {
        if let Some((_, token)) = &self.inner.lock().unwrap().active {
            token.cancel();
        }
    }

    pub fn read_response(&self, id: u64, offset: usize) -> Result<String, String> {
        let inner = self.inner.lock().unwrap();
        let (current, response) = inner.response.as_ref().ok_or("No response is available")?;
        if *current != id {
            return Err("This response is no longer available".into());
        }
        if offset > response.body.len() {
            return Err("Invalid body offset".into());
        }
        let end = offset.saturating_add(CHUNK_BYTES).min(response.body.len());
        Ok(base64::engine::general_purpose::STANDARD.encode(&response.body[offset..end]))
    }
}

#[cfg(test)]
mod tests;
