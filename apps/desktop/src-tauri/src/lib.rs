mod commands;
mod recovery;
mod session;
#[cfg(feature = "native-validation")]
mod validation;
mod watcher;
mod workspaces;

use session::Session;
use tauri::{Emitter, Manager};

pub fn run() {
    #[cfg(feature = "native-validation")]
    let started = std::time::Instant::now();
    #[cfg(feature = "native-validation")]
    let directory = validation::directory().expect("Could not open isolated validation directory");
    #[cfg(not(feature = "native-validation"))]
    let context = tauri::generate_context!();
    #[cfg(feature = "native-validation")]
    let context = {
        let mut context = tauri::generate_context!();
        for window in &mut context.config_mut().app.windows {
            window.incognito = cfg!(not(target_os = "macos"));
            #[cfg(not(target_os = "macos"))]
            {
                window.data_directory = Some(directory.join("webview"));
            }
        }
        context
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Session::default())
        .setup(move |app| {
            #[cfg(not(feature = "native-validation"))]
            let data = app.path().app_data_dir()?;
            #[cfg(feature = "native-validation")]
            let data = validation::setup(app, started, directory)?;
            app.state::<Session>()
                .init_history(data.join("history.json"));
            app.state::<Session>()
                .init_workspaces(data.join("workspaces.json"));
            app.state::<Session>()
                .init_recovery(data.join("recovery.json"));
            let handle = app.handle().clone();
            app.state::<Session>().init_watcher(move |change| {
                let _ = handle.emit("collection-changed", change);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::read_workspace,
            commands::refresh_collections,
            commands::inspect_request,
            commands::read_recovery,
            commands::save_recovery,
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
            commands::delete_request,
            commands::duplicate_request,
            commands::read_request,
            commands::read_request_summaries,
            commands::send_request,
            commands::cancel_request,
            commands::read_response,
            commands::release_response,
            commands::save_document,
            commands::read_environment,
            commands::read_variable_context,
            commands::create_environment,
            commands::read_history,
            commands::clear_history
        ])
        .run(context)
        .expect("Could not start NimblePost");
}
