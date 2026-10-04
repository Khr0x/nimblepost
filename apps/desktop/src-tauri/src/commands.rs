use crate::recovery::{RecoverySnapshot, RecoveryView};
use crate::session::{
    CollectionView, CreateRequestInput, HistoryView, RenameRequestInput, RequestInspection,
    RequestSummary, RequestUpdate, RequestView, ResponseMeta, SaveInput, SendInput, Session,
    WorkspaceView,
};
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) fn read_recovery(session: State<'_, Session>) -> RecoveryView {
    session.read_recovery()
}

#[tauri::command]
pub(crate) async fn save_recovery(
    snapshot: RecoverySnapshot,
    session: State<'_, Session>,
) -> Result<(), String> {
    session.save_recovery(snapshot).await
}

#[tauri::command]
pub(crate) async fn read_workspace(session: State<'_, Session>) -> Result<WorkspaceView, String> {
    session.read_workspace().await
}
#[tauri::command]
pub(crate) async fn refresh_collections(
    session: State<'_, Session>,
) -> Result<WorkspaceView, String> {
    session.refresh_collections().await
}
#[tauri::command]
pub(crate) async fn inspect_request(
    root: std::path::PathBuf,
    path: String,
    revision: u64,
    session: State<'_, Session>,
) -> Result<RequestInspection, String> {
    session.inspect_request(root, path, revision).await
}
#[tauri::command]
pub(crate) async fn create_workspace(
    name: String,
    session: State<'_, Session>,
) -> Result<WorkspaceView, String> {
    session.create_workspace(name).await
}
#[tauri::command]
pub(crate) async fn select_workspace(
    id: u32,
    session: State<'_, Session>,
) -> Result<WorkspaceView, String> {
    session.select_workspace(id).await
}
#[tauri::command]
pub(crate) async fn rename_workspace(
    id: u32,
    name: String,
    session: State<'_, Session>,
) -> Result<WorkspaceView, String> {
    session.rename_workspace(id, name).await
}
#[tauri::command]
pub(crate) async fn remove_workspace(
    id: u32,
    session: State<'_, Session>,
) -> Result<WorkspaceView, String> {
    session.remove_workspace(id).await
}
#[tauri::command]
pub(crate) async fn select_collection(
    root: std::path::PathBuf,
    session: State<'_, Session>,
) -> Result<CollectionView, String> {
    session.select_collection(root).await
}
#[tauri::command]
pub(crate) async fn remove_collection(
    root: std::path::PathBuf,
    session: State<'_, Session>,
) -> Result<WorkspaceView, String> {
    session.remove_collection(root).await
}

#[tauri::command]
pub(crate) async fn choose_collection(
    app: tauri::AppHandle,
    session: State<'_, Session>,
) -> Result<Option<CollectionView>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Open an OpenCollection folder")
        .pick_folder(move |folder| {
            let _ = sender.send(folder);
        });
    let folder = receiver
        .await
        .map_err(|_| "Folder picker closed unexpectedly")?;
    match folder {
        Some(path) => session
            .open(path.into_path().map_err(|_| "Invalid folder path")?)
            .await
            .map(Some),
        None => Ok(None),
    }
}

#[tauri::command]
pub(crate) async fn open_example(
    app: tauri::AppHandle,
    session: State<'_, Session>,
) -> Result<CollectionView, String> {
    let root = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("examples/basic-http");
    session.open_example(root).await
}

#[tauri::command]
pub(crate) async fn create_collection(
    app: tauri::AppHandle,
    session: State<'_, Session>,
    name: String,
    folder: String,
) -> Result<Option<CollectionView>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose where to create the collection")
        .pick_folder(move |folder| {
            let _ = sender.send(folder);
        });
    match receiver
        .await
        .map_err(|_| "Folder picker closed unexpectedly")?
    {
        Some(parent) => session
            .create_collection(
                parent.into_path().map_err(|_| "Invalid folder path")?,
                name,
                folder,
            )
            .await
            .map(Some),
        None => Ok(None),
    }
}

#[tauri::command]
pub(crate) async fn create_request(
    session: State<'_, Session>,
    input: CreateRequestInput,
) -> Result<RequestUpdate, String> {
    session.create_request(input).await
}

#[tauri::command]
pub(crate) async fn create_folder(
    session: State<'_, Session>,
    parent: String,
    name: String,
    folder: String,
) -> Result<CollectionView, String> {
    session.create_folder(parent, name, folder).await
}

#[tauri::command]
pub(crate) async fn rename_folder(
    session: State<'_, Session>,
    folder: String,
    name: String,
) -> Result<CollectionView, String> {
    session.rename_folder(folder, name).await
}

#[tauri::command]
pub(crate) async fn rename_request(
    session: State<'_, Session>,
    input: RenameRequestInput,
) -> Result<RequestUpdate, String> {
    session.rename_request(input).await
}

#[tauri::command]
pub(crate) async fn delete_request(
    session: State<'_, Session>,
    revision: u64,
) -> Result<CollectionView, String> {
    session.delete_request(revision).await
}

#[tauri::command]
pub(crate) async fn duplicate_request(
    session: State<'_, Session>,
    input: RenameRequestInput,
) -> Result<RequestUpdate, String> {
    session.duplicate_request(input).await
}

#[tauri::command]
pub(crate) async fn read_request_summaries(
    root: std::path::PathBuf,
    paths: Vec<String>,
    session: State<'_, Session>,
) -> Result<std::collections::BTreeMap<String, RequestSummary>, String> {
    session.request_summaries(root, paths).await
}

#[tauri::command]
pub(crate) async fn read_request(
    path: String,
    session: State<'_, Session>,
) -> Result<RequestView, String> {
    session.read_request(&path).await
}

#[tauri::command]
pub(crate) async fn send_request(
    input: SendInput,
    session: State<'_, Session>,
) -> Result<ResponseMeta, String> {
    session.send(input).await
}

#[tauri::command]
pub(crate) fn cancel_request(session: State<'_, Session>) {
    session.cancel();
}

#[tauri::command]
pub(crate) fn read_response(
    id: u64,
    offset: usize,
    session: State<'_, Session>,
) -> Result<String, String> {
    session.read_response(id, offset)
}

#[tauri::command]
pub(crate) fn release_response(id: u64, session: State<'_, Session>) {
    session.release_response(id);
}

#[tauri::command]
pub(crate) async fn save_document(
    input: SaveInput,
    session: State<'_, Session>,
) -> Result<RequestView, String> {
    session.save(input).await
}
#[tauri::command]
pub(crate) async fn read_variable_context(
    path: Option<String>,
    environment: Option<String>,
    session: State<'_, Session>,
) -> Result<Vec<nimblepost_core::VariablePreview>, String> {
    session
        .read_variable_context(path.as_deref(), environment.as_deref())
        .await
}
#[tauri::command]
pub(crate) async fn read_environment(
    name: String,
    session: State<'_, Session>,
) -> Result<RequestView, String> {
    session.read_environment(&name).await
}
#[tauri::command]
pub(crate) async fn create_environment(
    name: String,
    session: State<'_, Session>,
) -> Result<RequestView, String> {
    session.create_environment(name).await
}
#[tauri::command]
pub(crate) fn read_history(session: State<'_, Session>) -> HistoryView {
    session.history()
}
#[tauri::command]
pub(crate) async fn clear_history(session: State<'_, Session>) -> Result<(), String> {
    session.clear_history().await
}
