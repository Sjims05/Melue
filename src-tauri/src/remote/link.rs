//! This device controlling another (the app's side): one connection at a time. Its state comes to the app as
//! `remote-state` events, a command that didn't work there as `remote-error`, and the end of the connection as
//! `remote-closed` (with why, unless this device closed it).
//!
//! Connect device: [`remote_play_on`] moves what plays here to another device (and pauses here);
//! [`remote_bring_back`] moves what plays there back here (and pauses there).

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::control::{self, Command, Link, Outgoing, RemoteState};
use super::{key, presence, server};
use crate::library::Library;
use crate::sync::handoff::{self, Playback};
use tauri_plugin_native_audio::NativeAudioExt;

pub const STATE_EVENT: &str = "remote-state";
pub const ERROR_EVENT: &str = "remote-error";
pub const CLOSED_EVENT: &str = "remote-closed";

/// The connection (to which device, its name), the other device's state as it last said, and the queue this device
/// had when it sent it there (brought back as it was, songs only here included, when its song is in it).
#[derive(Default)]
pub struct Remote(
    Mutex<Option<(String, Link)>>,
    Mutex<Option<(Connected, RemoteState)>>,
    Mutex<Option<Vec<tauri_plugin_native_audio::QueueItem>>>,
);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connected {
    pub id: String,
    pub name: String,
    pub address: String,
}

/// Connects to one of your devices (closing any connection there was).
#[tauri::command]
pub async fn remote_connect(app: AppHandle, id: String) -> Result<Connected, String> {
    let blocking = app.clone();
    let wanted = id.clone();
    let (device, reached, key, me) = tauri::async_runtime::spawn_blocking(move || {
        let device = presence::device(&blocking, &wanted)?.ok_or("no such device")?;
        let reached = server::reach(&blocking, &device)?;
        Ok::<_, String>((device, reached, key::key(&blocking)?, handoff::device_id(&blocking.state::<Library>())?))
    })
    .await
    .map_err(|e| e.to_string())??;
    let port = device.port.ok_or("it doesn’t take connections")?;
    // One way at a time: the devices playing on this one are disconnected first (this one plays elsewhere now).
    if !control::controllers().is_empty() {
        let name = handoff::settings(&app).ok().and_then(|s| s.device_name).unwrap_or_else(|| "The other device".into());
        let target = device.name.clone();
        control::end_sessions_saying(|from| if from == id { format!("{name} plays here now") } else { format!("{name} plays on {target} now") });
    }
    let connected = Connected { id: id.clone(), name: device.name.clone(), address: reached.address.clone() };
    let (to_app, closing) = (app.clone(), app.clone());
    let (closed_id, keep) = (id.clone(), connected.clone());
    let link = control::connect(
        &reached.address,
        port,
        &key,
        &me,
        move |message| {
            let _ = match message {
                Outgoing::State(state) => {
                    *to_app.state::<Remote>().1.lock().unwrap() = Some((keep.clone(), state.clone()));
                    to_app.emit(STATE_EVENT, state)
                }
                Outgoing::Error(error) => to_app.emit(ERROR_EVENT, error),
            };
        },
        move |why| {
            // Only when it's still the connection there is (not one replaced or closed here).
            let remote = closing.state::<Remote>();
            let mut current = remote.0.lock().unwrap();
            if current.as_ref().is_some_and(|(id, _)| *id == closed_id) {
                *current = None;
                handoff::connection_ended();
                remote.1.lock().unwrap().take();
                remote.2.lock().unwrap().take();
                handoff::ignore(&closed_id, crate::library::now_ms() + 60_000);
                let _ = closing.emit(CLOSED_EVENT, why);
            }
        },
    )
    .await
    .map_err(|e| format!("{}: {e}", device.name))?;
    let remote = app.state::<Remote>();
    *remote.1.lock().unwrap() = None;
    // While playing there, Continue doesn't offer (or follow) that device's playback here, and rests altogether.
    handoff::ignore(&id, i64::MAX);
    if remote.0.lock().unwrap().replace((id, link)).is_none() {
        handoff::connection_began();
    }
    Ok(connected)
}

/// Connect device: plays what plays here on device `id` (from where it is) and pauses here. Nothing playing here:
/// just connected, to control what plays there.
#[tauri::command]
pub async fn remote_play_on(app: AppHandle, id: String) -> Result<Connected, String> {
    let connected = remote_connect(app.clone(), id).await?;
    let here = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let library = here.state::<Library>();
        let Some(folder) = library.synced_folder()? else { return Ok(()) };
        let Some((playback, _)) = handoff::playing_here(&here, &library, &folder)? else { return Ok(()) };
        remote_command(here.clone(), Command::Load { queue: playback.queue, index: playback.index, position: playback.position_s, playing: playback.playing })?;
        *here.state::<Remote>().2.lock().unwrap() = Some(here.native_audio().get_queue()?.items);
        here.native_audio().pause()
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(connected)
}

/// Connect device, "This device": what plays there comes here (its queue, the song, where in it; playing if it was),
/// it pauses there, and the connection closes.
#[tauri::command(async)]
pub fn remote_bring_back(app: AppHandle) -> Result<(), String> {
    let remote = app.state::<Remote>();
    let last = remote.1.lock().unwrap().clone();
    let sent = remote.2.lock().unwrap().take();
    if let Some((_, state)) = last.filter(|(_, s)| s.index >= 0) {
        let library = app.state::<Library>();
        let folder = library.synced_folder()?.ok_or("no share is set up")?;
        let playing_path = state.queue.get(state.index as usize).and_then(|song| song.path.clone());
        // The queue this device sent, when the song playing there is in it: as it was (songs only here too).
        let paths: std::collections::HashMap<i64, String> = library.synced_songs(&folder)?.into_iter().collect();
        let as_sent = sent.and_then(|items| {
            let at = items.iter().position(|item| item.id.and_then(|id| paths.get(&id)) == playing_path.as_ref())?;
            Some((items, at))
        });
        let (items, index) = match as_sent {
            Some(found) => found,
            None => {
                let queue: Vec<String> = state.queue.iter().filter_map(|song| song.path.clone()).collect();
                // The song's place among those on the share.
                let index = state.queue[..(state.index as usize).min(state.queue.len())].iter().filter(|song| song.path.is_some()).count();
                let wanted = Playback { queue, index, position_s: state.position, playing: state.playing, repeat: state.repeat.clone(), ..Default::default() };
                handoff::queue_here(&library, &folder, &wanted)?.ok_or("that song isn’t on this device (not synced yet?)")?
            }
        };
        let player = app.native_audio();
        player.set_queue(items, index, state.position, None)?;
        player.set_shuffle(state.shuffle)?;
        player.set_repeat_mode(&state.repeat)?;
        let _ = remote_command(app.clone(), Command::Pause);
        if state.playing {
            player.play()?;
        }
    }
    remote_disconnect(app);
    Ok(())
}

/// The songs here for the other device's queue (by their paths on the share): each one's id here, or None.
#[tauri::command(async)]
pub fn remote_song_ids(app: AppHandle, paths: Vec<Option<String>>) -> Result<Vec<Option<i64>>, String> {
    let library = app.state::<Library>();
    let Some(folder) = library.synced_folder()? else { return Ok(vec![None; paths.len()]) };
    let ids: std::collections::HashMap<String, i64> = library.synced_songs(&folder)?.into_iter().map(|(id, p)| (handoff::key(&p), id)).collect();
    Ok(paths.into_iter().map(|p| p.and_then(|p| ids.get(&handoff::key(&p)).copied())).collect())
}

/// Plays songs from here (a list, a song: by their ids here) on the device connected to, from song `index`. Songs
/// that aren't on the share can't play there: left out.
#[tauri::command(async)]
pub fn remote_load(app: AppHandle, ids: Vec<i64>, index: usize, playing: bool) -> Result<(), String> {
    let library = app.state::<Library>();
    let folder = library.synced_folder()?.ok_or("no share is set up")?;
    let paths: std::collections::HashMap<i64, String> = library.synced_songs(&folder)?.into_iter().collect();
    let start = *ids.get(index).ok_or("no such song")?;
    if !paths.contains_key(&start) {
        return Err("that song is only on this device, so it can’t play on the other one".into());
    }
    let index = ids[..index].iter().filter(|id| paths.contains_key(id)).count();
    let queue = ids.iter().filter_map(|id| paths.get(id).cloned()).collect();
    remote_command(app.clone(), Command::Load { queue, index, position: 0.0, playing })
}

/// Whether this device plays on another now: which, and that one's state as it last said (None: playing here).
#[tauri::command]
pub fn remote_status(app: AppHandle) -> Option<(Connected, Option<RemoteState>)> {
    let remote = app.state::<Remote>();
    let connected = remote.0.lock().unwrap().as_ref().map(|(id, _)| id.clone())?;
    let last = remote.1.lock().unwrap().clone();
    Some(match last {
        Some((who, state)) if who.id == connected => (who, Some(state)),
        _ => (Connected { id: connected, name: String::new(), address: String::new() }, None),
    })
}

/// A command to the device connected to.
#[tauri::command]
pub fn remote_command(app: AppHandle, command: Command) -> Result<(), String> {
    let remote = app.state::<Remote>();
    let current = remote.0.lock().unwrap();
    let (_, link) = current.as_ref().ok_or("not connected to another device")?;
    link.send(command)
}

/// What plays here, sent to the device connected to (its queue, the song and where in it; playing or paused).
#[tauri::command(async)]
pub fn remote_send_playing(app: AppHandle) -> Result<(), String> {
    let library = app.state::<Library>();
    let folder = library.synced_folder()?.ok_or("no share is set up")?;
    let (playback, _) = handoff::playing_here(&app, &library, &folder)?.ok_or("nothing that’s on the share plays here")?;
    remote_command(
        app.clone(),
        Command::Load { queue: playback.queue, index: playback.index, position: playback.position_s, playing: playback.playing },
    )
}

/// Another device started playing on this one: this one's own connection closes (one way at a time), the app told
/// why (`remote-closed`).
pub fn give_way(app: &AppHandle, to: &str) {
    if app.state::<Remote>().0.lock().unwrap().is_some() {
        remote_disconnect(app.clone());
        let _ = app.emit(CLOSED_EVENT, format!("{to} plays here now"));
    }
}

/// Closes the connection (the other device goes on as it is).
#[tauri::command]
pub fn remote_disconnect(app: AppHandle) {
    // Dropping the link closes it.
    let remote = app.state::<Remote>();
    if let Some((id, _)) = remote.0.lock().unwrap().take() {
        handoff::connection_ended();
        // A minute more: what that device saves as it stops isn't offered back here.
        handoff::ignore(&id, crate::library::now_ms() + 60_000);
    }
    remote.1.lock().unwrap().take();
    remote.2.lock().unwrap().take();
}
