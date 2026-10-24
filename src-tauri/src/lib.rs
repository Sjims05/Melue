#[cfg(desktop)]
mod desktop;
mod dev;
mod connections;
mod discord;
mod github;
mod library;
mod logging;
mod phone_update;
mod remote;
mod report;
mod share;
mod startup;
mod sync;


/// Who this Melue is (its data folders, its log, its saved passwords): the official app, or a development build as
/// "Melue Dev", which keeps its own (never the installed app's library or settings). Android test builds are their own
/// app too (com.simon.melue.dev; build.gradle.kts).
pub const IDENTITY: &str = if cfg!(debug_assertions) { "com.simon.melue.dev" } else { "com.simon.melue" };

/// Whether this is Melue Dev (a development build, or an Android test build): it never updates itself (lib/updates.ts
/// goes through the steps without installing anything).
#[tauri::command]
fn app_is_dev() -> bool {
    #[cfg(target_os = "android")]
    return logging::android_package().ends_with(".dev");
    #[cfg(not(target_os = "android"))]
    cfg!(debug_assertions)
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Android: Melue's messages to Android's log and a log file on the phone (logging.rs).
    logging::start();
    startup::begin();
    let builder = tauri::Builder::default();

    // Android only (see Cargo.toml): android-fs (its permissions are in capabilities/android.json), and
    // Melue's own Android code (no JavaScript commands, used by the library).
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_android_fs::init()).plugin(tauri_plugin_melue_android::init());
    // Desktop only: one Melue at a time (registered first, so a second start goes no further), and starting with
    // the system (desktop.rs).
    #[cfg(desktop)]
    let builder = builder
        .plugin(desktop::single_instance())
        .plugin(desktop::autostart())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_native_audio::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_opener::init())
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
            // Desktop: the window first (desktop.rs), so a slow start-up never leaves it hidden.
            #[cfg(desktop)]
            desktop::show_at_start(app.handle());
            // The music library, in the app's data folder.
            let library = library::Library::open(&app.path().app_data_dir()?.join("library.db"))?;
            // A fresh install on a computer starts with the Music folder (folders.rs).
            #[cfg(desktop)]
            if let Err(e) = library.add_music_folder_once(app.path().audio_dir().ok().as_deref()) {
                eprintln!("melue: can't add the Music folder: {e}");
            }
            app.manage(library);
            // Scanning: once now (start-up, in the background), and after changes on Android.
            app.manage(library::scanner::Scanner::default());
            library::scanner::start(app.handle());
            // Android Auto's library: sent now and after changes (Android only).
            app.manage(library::auto::AutoSender::default());
            library::auto::start(app.handle());
            // Lyrics for songs without their own, found online on Wi-Fi (online_lyrics.rs).
            app.manage(library::online_lyrics::LyricsLookup::default());
            library::online_lyrics::start(app.handle());
            // Syncing with the share by itself (start-up, coming back, the share's changes; see sync/schedule.rs).
            sync::scheduler::start(app.handle());
            // Bug reports waiting to be sent: tried a while after start (then at each library sync).
            report::saved::start(app.handle());
            // Synced playback: what plays here goes to the share; another device's is offered (see sync/handoff.rs).
            app.manage(sync::shared::OpenShare::default());
            app.manage(remote::link::Remote::default());
            // Remote playback (after the share's state above, which it uses): this device says it's online every 20 s
            // (remote/presence.rs), and takes connections from your other devices (remote/server.rs).
            remote::presence::start(app.handle());
            remote::server::start(app.handle());
            // Playing on another device, the phone's lock screen, notification and volume buttons control it.
            remote::media::start(app.handle());
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
            app.manage(sync::to_share::ToShare::default());
            sync::data::start(app.handle());
            library::android::on_pause(app.handle(), {
                let app = app.handle().clone();
                move || sync::scheduler::went_away(&app)
            });
            library::android::on_resume(app.handle(), {
                let app = app.handle().clone();
                move || {
                    sync::scheduler::trigger(&app, sync::schedule::Trigger::Return);
                    sync::handoff::check(&app);
                }
            });
            library::android::on_network_change(app.handle(), {
                let app = app.handle().clone();
                move || {
                    sync::scheduler::network_changed(&app);
                    library::online_lyrics::soon(&app);
                }
            });
            library::android::on_sync_cancel(app.handle(), {
                let app = app.handle().clone();
                move || sync::scheduler::cancel(&app)
            });
            // Desktop: the tray, starting with the system (desktop.rs).
            #[cfg(desktop)]
            desktop::setup(app.handle())?;
            // Discord: "Listening to Melue" (when switched on).
            #[cfg(desktop)]
            discord::start(app.handle());
            eprintln!("melue: start-up: set up at {} ms", startup::ms());
            Ok(())
        })
        // Desktop: back into the window counts as coming back to the app (a sync if the last is a while ago).
        .on_window_event(|window, event| {
            use tauri::Manager;
            match event {
                // Leaving the window: coming back after a while syncs (Sync when coming back after…).
                tauri::WindowEvent::Focused(false) => sync::scheduler::went_away(window.app_handle()),
                tauri::WindowEvent::Focused(true) => {
                    sync::scheduler::trigger(window.app_handle(), sync::schedule::Trigger::Return);
                    sync::handoff::check(window.app_handle());
                }
                // Closing: on desktop usually hidden in the tray (desktop.rs); when it quits, where the song is now
                // goes to the share.
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    #[cfg(desktop)]
                    if desktop::on_close(window, api) {
                        return;
                    }
                    let _ = api;
                    sync::handoff::save_now(window.app_handle());
                    // The other devices hear at once that this one is gone.
                    remote::presence::leave(window.app_handle());
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            startup::window_ready,
            phone_update::update_latest_release,
            phone_update::update_install_apk,
            phone_update::update_install_downloaded,
            logging::log_show,
            app_is_dev,
            remote::remote_settings,
            remote::remote_set_settings,
            remote::presence::remote_devices,
            remote::presence::remote_forget_device,
            remote::server::remote_reach,
            remote::link::remote_connect,
            remote::link::remote_command,
            remote::link::remote_disconnect,
            remote::link::remote_send_playing,
            remote::link::remote_play_on,
            remote::link::remote_bring_back,
            remote::link::remote_status,
            remote::link::remote_song_ids,
            remote::link::remote_load,
            remote::link::remote_play_list,
            remote::link::remote_add,
            remote::server::remote_answer,
            remote::server::remote_controlled_by,
            remote::server::remote_end_control,
            dev::dev_share_sample,
            github::github_sign_in_start,
            github::github_sign_in_wait,
            github::github_sign_in_cancel,
            github::github_account,
            github::github_sign_out,
            github::github_create_issue,
            report::scrub::report_scrub,
            report::scrub::report_log,
            report::saved::report_save,
            report::saved::report_discard,
            report::saved::reports_list,
            report::screenshot::report_screenshot,
            share::commands::share_info,
            share::commands::share_test,
            share::commands::share_set_up,
            share::commands::share_set_password,
            share::commands::share_remove,
            share::commands::share_set_shown_in_other_apps,
            share::commands::share_set_home_host,
            sync::commands::sync_plan,
            sync::to_share::sync_to_share,
            sync::to_share::sync_put_on_share,
            sync::commands::sync_now,
            sync::commands::sync_cancel,
            sync::commands::sync_came_back,
            sync::commands::sync_mobile_use,
            sync::commands::shared_settings_checked,
            sync::commands::sync_status,
            sync::commands::sync_simulate_notification,
            sync::commands::sync_settings,
            sync::commands::sync_set_settings,
            sync::commands::playback_sync_settings,
            sync::commands::playback_stand_in,
            sync::commands::playback_stand_in_done,
            sync::commands::playback_sync_set_settings,
            sync::commands::playback_offer,
            sync::commands::playback_offer_song,
            sync::commands::playback_continue,
            sync::commands::playback_not_now,
            sync::commands::playback_check,
            sync::commands::library_sync_now,
            sync::commands::sync_not_on_share,
            sync::commands::sync_move_to_sync,
            sync::commands::sync_copies,
            sync::commands::shared_settings,
            sync::commands::record_shared_settings,
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
            library::library_missing_songs,
            library::library_forget_missing,
            discord::discord_settings,
            discord::discord_set_settings,
            discord::discord_status,
            library::library_lyrics,
            library::library_set_lyrics_timing,
            library::library_lyrics_other_version,
            library::library_share_folders,
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
            library::library_player_settings,
            library::library_tracks_by_ids,
            library::library_queue_items_for,
            library::library_let_go_files,
            library::library_follow_file_again,
            library::library_set_playlist_following,
            library::library_forget_let_go_file,
            library::library_playlists_with_track,
            library::library_search,
            library::library_set_playlist_cover,
            library::library_clear_playlist_cover,
            library::library_read_picture,
            library::library_add_songs_to_folder,
            library::library_export_playlist,
            library::library_deleted_linked_playlists,
            library::library_restore_linked_playlist,
            library::library_album_page,
            library::library_artist_page,
            library::library_folder_tracks,
            library::library_auto_playlist_tracks,
            library::library_show_in_folder,
            library::library_covers_size,
            library::library_clean_covers,
            library::library_set_player_settings,
            library::library_folder_covers,
            library::library_set_folder_covers,
            library::songs_there::library_songs_there,
        ])
        .run(context())
        .expect("error while running tauri application");
}

/// tauri.conf.json's app; a development build on a computer is Melue Dev (see [`IDENTITY`]).
fn context() -> tauri::Context {
    #[allow(unused_mut)]
    let mut context = tauri::generate_context!();
    #[cfg(all(desktop, debug_assertions))]
    {
        context.config_mut().identifier = IDENTITY.to_string();
        context.config_mut().product_name = Some("Melue Dev".to_string());
        context.package_info_mut().name = "Melue Dev".to_string();
        for window in &mut context.config_mut().app.windows {
            window.title = "Melue Dev".to_string();
        }
    }
    context
}
