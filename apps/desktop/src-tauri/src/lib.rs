mod commands;
mod session;
mod workspaces;

use session::Session;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Session::default())
        .setup(|app| {
            app.state::<Session>()
                .init_history(app.path().app_data_dir()?.join("history.json"));
            app.state::<Session>()
                .init_workspaces(app.path().app_data_dir()?.join("workspaces.json"));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::read_workspace,
            commands::create_workspace,
            commands::select_workspace,
            commands::rename_workspace,
            commands::remove_workspace,
            commands::select_collection,
            commands::remove_collection,
            commands::choose_collection,
            commands::open_example,
            commands::create_collection,
            commands::create_request,
            commands::create_folder,
            commands::rename_folder,
            commands::rename_request,
            commands::duplicate_request,
            commands::read_request,
            commands::read_request_summaries,
            commands::send_request,
            commands::cancel_request,
            commands::read_response,
            commands::save_document,
            commands::read_environment,
            commands::create_environment,
            commands::read_history,
            commands::clear_history
        ])
        .run(tauri::generate_context!())
        .expect("Could not start NimblePost");
}
