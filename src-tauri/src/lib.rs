mod library;
mod share;

use std::sync::{Arc, Mutex};

use share::smb::{SmbConfig, SmbShare};
use share::{Entry, Share};

/// The spike's SMB share (section 1, with .env's test login), through Phase 2's share access.
#[derive(Default)]
struct SpikeShare(Mutex<Option<Arc<SmbShare>>>);

impl SpikeShare {
    fn get(&self) -> Result<Arc<SmbShare>, String> {
        self.0.lock().unwrap().clone().ok_or_else(|| "not connected: connect first".to_string())
    }
}

/// Runs share work on a thread meant for waiting, not one of Tauri's async threads.
async fn on_share_thread<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|e| e.to_string())?
}

/// Connects to the share (its root folder) and checks it can be listed.
#[tauri::command]
async fn smb_connect(state: tauri::State<'_, SpikeShare>, host: String, user: String, pass: String, share: String) -> Result<(), String> {
    let connected = on_share_thread(move || {
        let connected = SmbShare::new(SmbConfig { host, share, folder: String::new(), user, password: pass })?;
        connected.check()?;
        Ok(connected)
    })
    .await?;
    *state.0.lock().unwrap() = Some(Arc::new(connected));
    Ok(())
}

#[tauri::command]
async fn smb_list_dir(state: tauri::State<'_, SpikeShare>, path: String) -> Result<Vec<Entry>, String> {
    let share = state.get()?;
    on_share_thread(move || Ok(share.list(&path)?)).await
}

/// Downloads a file to `local_path` (a temporary file first, renamed when complete). Returns its size.
#[tauri::command]
async fn smb_copy_file(state: tauri::State<'_, SpikeShare>, remote_path: String, local_path: String) -> Result<u64, String> {
    let share = state.get()?;
    on_share_thread(move || {
        let local = std::path::PathBuf::from(&local_path);
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let part = std::path::PathBuf::from(format!("{local_path}.part"));
        let copied = (|| -> Result<u64, String> {
            let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
            let size = share.read_to(&remote_path, &mut file, &mut |_| {})?;
            file.sync_all().map_err(|e| e.to_string())?;
            Ok(size)
        })();
        match copied.and_then(|size| std::fs::rename(&part, &local).map(|_| size).map_err(|e| e.to_string())) {
            Ok(size) => Ok(size),
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                Err(e)
            }
        }
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // Android only (see Cargo.toml): android-fs (its permissions are in capabilities/android.json), and
    // Melue's own Android code (no JavaScript commands, used by the library).
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_android_fs::init()).plugin(tauri_plugin_melue_android::init());

    builder
        .plugin(tauri_plugin_native_audio::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_opener::init())
        .manage(SpikeShare::default())
        // Covers for the UI: cover://localhost/<file>-300.jpg (src/library's coverUrl makes these).
        .register_asynchronous_uri_scheme_protocol("cover", |ctx, request, responder| {
            use tauri::Manager;
            let covers = ctx.app_handle().state::<library::Library>().covers_dir().to_path_buf();
            let path = request.uri().path().to_string();
            tauri::async_runtime::spawn_blocking(move || responder.respond(library::covers::serve(&covers, &path)));
        })
        .setup(|app| {
            use tauri::Manager;
            // The music library, in the app's data folder.
            let library = library::Library::open(&app.path().app_data_dir()?.join("library.db"))?;
            app.manage(library);
            // Scanning: once now (start-up, in the background), and after changes on Android.
            app.manage(library::scanner::Scanner::default());
            library::scanner::start(app.handle());
            // Android Auto's library: sent now and after changes (Android only).
            app.manage(library::auto::AutoSender::default());
            library::auto::start(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            smb_connect,
            smb_list_dir,
            smb_copy_file,
            library::library_status,
            library::library_setting,
            library::library_set_setting,
            library::library_sync_available,
            library::library_folders,
            library::library_add_folder,
            library::library_remove_folder,
            library::library_set_folder_enabled,
            library::library_rename_folder,
            library::library_music_access,
            library::library_scan,
            library::library_tracks,
            library::library_albums,
            library::library_artists,
            library::library_playlists,
            library::library_playlist,
            library::library_playlist_entries,
            library::library_create_playlist,
            library::library_rename_playlist,
            library::library_delete_playlist,
            library::library_add_to_playlist,
            library::library_remove_from_playlist,
            library::library_move_in_playlist,
            library::library_restore_playlist_entry,
            library::library_set_playlist_options,
            library::library_unlink_playlist,
            library::library_queue_items,
            library::library_record_playback,
            library::library_set_liked,
            library::library_liked_ids,
            library::library_recently_played,
            library::library_most_played,
            library::library_queue_follow,
            library::library_set_queue_follow,
            library::library_folder_covers,
            library::library_set_folder_covers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
