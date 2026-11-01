//! A song counts as played as soon as it has played long enough (playback.rs `outcome`), not only once it's left:
//! whatever's done after, it was a play, and Most played, Top songs and the other devices see it at once. Checked
//! every few seconds while something plays. With the app not running (Android Auto), it's counted when it's left.

use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_native_audio::NativeAudioExt;

use super::changes::{emit, LibraryChange};
use super::Library;

/// How often the playing song is looked at.
const EVERY: Duration = Duration::from_secs(5);

/// Starts looking at the playing song.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("melue-played".into()).spawn(move || loop {
        std::thread::sleep(EVERY);
        let Ok(state) = app.native_audio().get_state() else { continue };
        let (true, Some(track)) = (state.is_playing, state.current_id) else { continue };
        match app.state::<Library>().count_play_now(track, (state.current_time * 1000.0) as i64) {
            Ok(true) => {
                emit(&app, LibraryChange::played(track));
                super::auto::send_soon(&app);
                crate::sync::data::soon(&app);
            }
            Ok(false) => {}
            Err(e) => eprintln!("melue: can't count the play: {e}"),
        }
    });
}
