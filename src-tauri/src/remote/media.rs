//! The phone's own media controls while it plays on another device (Connect device): the lock screen, the
//! notification and the volume buttons show what plays there and control it (native-audio's remote playback). The
//! volume buttons move that device's volume in steps: a phone's ("Volume steps on a phone"), or a computer's
//! ("Volume buttons on another device"). On a computer, nothing: its media keys control its own player.

use tauri::AppHandle;

use super::control::RemoteState;
use super::link::Connected;

/// Setup: the controls' presses go to the device played on.
pub fn start(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use super::control::Command;
        use tauri_plugin_native_audio::{NativeAudioExt, RemoteCommand};
        let to = app.clone();
        let listening = app.native_audio().on_remote_command(move |command| {
            let command = match command {
                RemoteCommand::Play => Command::Play,
                RemoteCommand::Pause => Command::Pause,
                RemoteCommand::Next => Command::Next,
                RemoteCommand::Previous => Command::Previous,
                RemoteCommand::Seek { position } => Command::Seek { position },
                RemoteCommand::Volume { step, max_volume } => Command::Volume { volume: f64::from(step) / f64::from(max_volume.max(1)) },
                RemoteCommand::Shuffle { on } => Command::Shuffle { on },
                RemoteCommand::Repeat { mode } => Command::Repeat { mode },
            };
            if let Err(e) = super::link::remote_command(to.clone(), command) {
                eprintln!("melue: a control for the other device didn't go: {e}");
            }
        });
        if let Err(e) = listening {
            eprintln!("melue: the phone's controls can't control other devices: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}

/// The phone's controls show what plays on the device connected to (its state as it last said).
pub fn show(app: &AppHandle, connected: &Connected, state: &RemoteState) {
    #[cfg(target_os = "android")]
    {
        let (app, connected, state) = (app.clone(), connected.clone(), state.clone());
        tauri::async_runtime::spawn_blocking(move || {
            // One at a time, and only while still connected (not after the connection ended).
            let _one = SHOWING.lock().unwrap();
            if !super::link::connected(&app) {
                return;
            }
            let playback = playback(&app, &connected, &state);
            if let Err(e) = tauri_plugin_native_audio::NativeAudioExt::native_audio(&app).set_remote_playback(Some(playback)) {
                eprintln!("melue: the phone's controls can't show the other device: {e}");
            }
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, connected, state);
}

/// The connection's over: the phone's controls are its own player's again.
pub fn hide(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _one = SHOWING.lock().unwrap();
            let _ = tauri_plugin_native_audio::NativeAudioExt::native_audio(&app).set_remote_playback(None);
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}

#[cfg(target_os = "android")]
static SHOWING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// What the phone's controls show: the song playing there (its cover from here, when it's here), its shuffle and
/// repeat, and its volume in the steps the volume buttons move it by.
#[cfg(target_os = "android")]
fn playback(app: &AppHandle, connected: &Connected, state: &RemoteState) -> tauri_plugin_native_audio::RemotePlayback {
    use tauri::Manager;
    let song = usize::try_from(state.index).ok().and_then(|i| state.queue.get(i));
    // The song here (when it's synced): its cover, and the like button is for it.
    let item_id = song.and_then(|s| s.path.clone()).and_then(|path| super::link::remote_song_ids(app.clone(), vec![Some(path)]).ok()?.into_iter().next().flatten());
    let artwork_url = item_id.and_then(|id| app.state::<crate::library::Library>().queue_items_for(&[id]).ok()?.into_iter().next()?.artwork_url);
    let at = state.play_order.iter().position(|&i| i == state.index);
    let has_next = state.repeat != "off" || at.map_or(state.index + 1 < state.queue.len() as i64, |at| at + 1 < state.play_order.len());
    let max_volume = volume_steps(app, state.volume_levels);
    tauri_plugin_native_audio::RemotePlayback {
        device: connected.name.clone(),
        title: song.and_then(|s| s.title.clone()),
        artist: song.and_then(|s| s.artist.clone()),
        artwork_url,
        playing: state.playing,
        position: state.position,
        duration: state.duration,
        has_next,
        volume: (state.volume * f64::from(max_volume)).round() as u32,
        max_volume,
        shuffle: state.shuffle,
        repeat: state.repeat.clone(),
        item_id,
    }
}

/// The volume buttons' steps on the other device, with this device's settings.
#[cfg(target_os = "android")]
fn volume_steps(app: &AppHandle, levels: u32) -> u32 {
    let settings = super::settings(app).ok();
    steps(levels, settings.as_ref().map_or(10, |s| s.phone_step), settings.as_ref().map_or(5, |s| s.button_step))
}

/// The other device's volume in steps: a phone's `levels` (> 0), `phone_step` of them a step (one each with 30 or
/// fewer: they're like its buttons already); a computer's 0 to 100%, `button_step` % a step.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn steps(levels: u32, phone_step: u32, button_step: u32) -> u32 {
    if levels > 30 {
        (f64::from(levels) / f64::from(phone_step.max(1))).round().max(1.0) as u32
    } else if levels > 0 {
        levels
    } else {
        100 / button_step.clamp(1, 100)
    }
}

#[cfg(test)]
mod tests {
    use super::steps;

    #[test]
    fn the_volume_buttons_step_like_the_settings_say() {
        // A phone with fine volume: 160 levels, 10 a step (its own buttons' step).
        assert_eq!(steps(160, 10, 5), 16);
        assert_eq!(steps(150, 20, 5), 8);
        // A phone with few levels: each one.
        assert_eq!(steps(15, 10, 5), 15);
        // A computer: 5% a press.
        assert_eq!(steps(0, 10, 5), 20);
        assert_eq!(steps(0, 10, 2), 50);
    }
}
