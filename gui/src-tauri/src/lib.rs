mod bookings;
mod cinemas;
#[cfg(test)]
mod live_check;
mod outbox;
mod settings;
mod state;
#[cfg(test)]
mod test_fixtures;
mod titles;

const APP_DIRECTORY_NAME: &str = "kdmcourier";

fn data_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .expect("no data directory, HOME is not set")
        .join(APP_DIRECTORY_NAME)
}

// the commands the page calls, shared with the IPC test so both register the same list
pub fn with_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        settings::settings_load,
        settings::settings_save,
        settings::time_zones,
        titles::titles_list,
        titles::titles_import_dkdm,
        titles::titles_set_standard,
        cinemas::cinemas_list,
        cinemas::cinemas_import_flm,
        cinemas::cinemas_update,
        cinemas::cinemas_remove,
        cinemas::cinemas_import_dcpwizard,
        cinemas::history_import_dcpwizard,
        bookings::bookings_list,
        bookings::bookings_add,
        bookings::bookings_update,
        bookings::bookings_remove,
        bookings::bookings_plan,
        bookings::bookings_issue,
        outbox::outbox_list,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    guikit_startup::prefer_shared_memory_webkit_frames_on_nvidia();
    #[cfg(unix)]
    guikit_startup::fork_terminal_guard();

    let state = state::AppState::new(settings::settings_path(), data_dir());
    with_commands(tauri::Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
