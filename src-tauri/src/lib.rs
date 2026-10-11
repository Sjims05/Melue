mod library;
mod logging;
mod share;
mod sync;

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

use share::commands::on_share_thread;

/// Connects to the share (its root folder) and checks it can be listed.
#[tauri::command]
async fn smb_connect(state: tauri::State<'_, SpikeShare>, host: String, user: String, pass: String, share: String) -> Result<(), String> {
    let connected = on_share_thread(move || {
        let connected = SmbShare::new(SmbConfig { host, share, user, password: pass, ..Default::default() })?;
        connected.check()?;
        Ok::<_, String>(connected)
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
    // Android: Melue's messages to Android's log and a log file on the phone (logging.rs).
    logging::start();
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
        .manage(share::commands::SessionPasswords::default())
        .manage(sync::scheduler::Syncing::default())
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
            // Syncing with the share by itself (start-up, coming back, the share's changes; see sync/schedule.rs).
            sync::scheduler::start(app.handle());
            // Synced playback: what plays here goes to the share; another device's is offered (see sync/handoff.rs).
            app.manage(sync::shared::OpenShare::default());
            // The car with Melue closed finds the library here (sync/car.rs).
            #[cfg(target_os = "android")]
            {
                use tauri_plugin_melue_android::MelueAndroidExt;
                let dir = app.path().app_data_dir()?;
                if let Err(e) = app.melue_android().remember_data_dir(&dir.to_string_lossy()) {
                    eprintln!("melue: can't remember the data folder for the car: {e}");
                }
            }
            app.manage(sync::handoff::Handoff::default());
            sync::handoff::start(app.handle());
            // Library data across devices: likes, plays and playlists (see sync/data.rs).
            app.manage(sync::data::LibrarySync::default());
            sync::data::start(app.handle());
            library::android::on_resume(app.handle(), {
                let app = app.handle().clone();
                move || {
                    sync::scheduler::trigger(&app, sync::schedule::Trigger::Return);
                    sync::handoff::check(&app);
                }
            });
            library::android::on_network_change(app.handle(), {
                let app = app.handle().clone();
                move || sync::scheduler::network_changed(&app)
            });
            library::android::on_sync_cancel(app.handle(), {
                let app = app.handle().clone();
                move || sync::scheduler::cancel(&app)
            });
            Ok(())
        })
        // Desktop: back into the window counts as coming back to the app (a sync if the last is a while ago).
        .on_window_event(|window, event| {
            use tauri::Manager;
            match event {
                tauri::WindowEvent::Focused(true) => {
                    sync::scheduler::trigger(window.app_handle(), sync::schedule::Trigger::Return);
                    sync::handoff::check(window.app_handle());
                }
                // Closing: where the song is now goes to the share.
                tauri::WindowEvent::CloseRequested { .. } => sync::handoff::save_now(window.app_handle()),
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            smb_connect,
            smb_list_dir,
            smb_copy_file,
            share::commands::share_info,
            share::commands::share_test,
            share::commands::share_set_up,
            share::commands::share_set_password,
            share::commands::share_remove,
            share::commands::share_set_shown_in_other_apps,
            share::commands::share_set_home_host,
            sync::commands::sync_plan,
            sync::commands::sync_now,
            sync::commands::sync_cancel,
            sync::commands::sync_status,
            sync::commands::sync_simulate_notification,
            sync::commands::sync_settings,
            sync::commands::sync_set_settings,
            sync::commands::playback_sync_settings,
            sync::commands::playback_sync_set_settings,
            sync::commands::playback_offer,
            sync::commands::playback_continue,
            sync::commands::playback_not_now,
            sync::commands::playback_check,
            sync::commands::library_sync_now,
            sync::commands::sync_not_on_share,
            sync::commands::sync_move_to_sync,
            sync::commands::sync_copies,
            sync::commands::sync_clear_copies,
            sync::commands::sync_keep_copies,
            sync::commands::sync_check_copies,
            sync::commands::sync_layout_status,
            sync::commands::sync_reorganise,
            sync::commands::playlist_conflicts,
            sync::commands::playlist_answer_conflict,
            sync::commands::deleted_playlists,
            sync::commands::restore_playlist,
            sync::commands::set_playlist_local_only,
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
            library::library_auto_playlists,
            library::library_set_auto_playlist,
            library::library_queue_follow,
            library::library_set_queue_follow,
            library::library_folder_covers,
            library::library_set_folder_covers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
