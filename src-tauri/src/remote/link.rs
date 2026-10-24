//! This device controlling another (the app's side): one connection at a time. Its state comes to the app as
//! `remote-state` events, a command that didn't work there as `remote-error`, and the end of the connection as
//! `remote-closed` (with why, unless this device closed it). A connection lost (not ended by either side) is tried
//! again for a minute (`remote-reconnecting`), the other device playing on meanwhile.
//!
//! Connect device: [`remote_play_on`] moves what plays here to another device (and pauses here);
//! [`remote_bring_back`] moves what plays there back here (and pauses there).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::control::{self, Closed, Command, Link, Outgoing, RemoteState};
use super::presence::Device;
use super::{key, media, presence, server};
use crate::library::Library;
use crate::sync::handoff::{self, Playback};
use tauri_plugin_native_audio::NativeAudioExt;

pub const STATE_EVENT: &str = "remote-state";
pub const ERROR_EVENT: &str = "remote-error";
pub const CLOSED_EVENT: &str = "remote-closed";
/// The connection was lost and is being tried again (true), or is back (false).
pub const RECONNECTING_EVENT: &str = "remote-reconnecting";

/// The connection (to which device, its name), the other device's state as it last said, and the queue this device
/// had when it sent it there (brought back as it was, songs only here included, when its song is in it).
#[derive(Default)]
pub struct Remote(Mutex<Option<Current>>, Mutex<Option<(Connected, RemoteState)>>, Mutex<Option<Vec<tauri_plugin_native_audio::QueueItem>>>);

/// The connection there is: to which device, which connection it is (its serial: one replaced, or closed here,
/// doesn't end the next), and its link (None while reconnecting).
pub struct Current {
    id: String,
    serial: u64,
    link: Option<Link>,
}

static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// A lost connection: tried again this often...
const RETRY_EVERY: Duration = Duration::from_secs(3);
/// ...for this long, then given up.
const RECONNECT_FOR: Duration = Duration::from_secs(60);

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
    let device = {
        let (blocking, wanted) = (app.clone(), id.clone());
        tauri::async_runtime::spawn_blocking(move || presence::device(&blocking, &wanted)).await.map_err(|e| e.to_string())??.ok_or("no such device")?
    };
    // One way at a time: the devices playing on this one are disconnected first (this one plays elsewhere now).
    if !control::controllers().is_empty() {
        let name = handoff::settings(&app).ok().and_then(|s| s.device_name).unwrap_or_else(|| "The other device".into());
        let target = device.name.clone();
        control::end_sessions_saying(|from| if from == id { format!("{name} plays here now") } else { format!("{name} plays on {target} now") });
    }
    let serial = NEXT_SERIAL.fetch_add(1, Ordering::SeqCst);
    // The last device's state goes before this connection opens (its first state can come in at once).
    *app.state::<Remote>().1.lock().unwrap() = None;
    let (connected, link) = open(&app, &id, serial).await?;
    let remote = app.state::<Remote>();
    // While playing there, Continue doesn't offer (or follow) that device's playback here, and rests altogether.
    handoff::ignore(&id, i64::MAX);
    if remote.0.lock().unwrap().replace(Current { id, serial, link: Some(link) }).is_none() {
        handoff::connection_began();
    }
    Ok(connected)
}

/// Opens connection `serial` to device `id`: its state goes to the app, and its end to [`closed`].
async fn open(app: &AppHandle, id: &str, serial: u64) -> Result<(Connected, Link), String> {
    let blocking = app.clone();
    let wanted = id.to_string();
    let (device, reached, key, me) = tauri::async_runtime::spawn_blocking(move || {
        let device = presence::device(&blocking, &wanted)?.ok_or("no such device")?;
        let reached = server::reach(&blocking, &device)?;
        Ok::<_, String>((device, reached, key::key(&blocking)?, handoff::device_id(&blocking.state::<Library>())?))
    })
    .await
    .map_err(|e| e.to_string())??;
    let port = device.port.ok_or("it doesn’t take connections")?;
    let connected = Connected { id: id.to_string(), name: device.name.clone(), address: reached.address.clone() };
    let (to_app, closing, keep) = (app.clone(), app.clone(), connected.clone());
    let link = control::connect(
        &reached.address,
        port,
        &key,
        &me,
        move |message| {
            let _ = match message {
                Outgoing::State(state) => {
                    *to_app.state::<Remote>().1.lock().unwrap() = Some((keep.clone(), state.clone()));
                    // The phone's lock screen, notification and volume buttons follow it (Android).
                    media::show(&to_app, &keep, &state);
                    to_app.emit(STATE_EVENT, state)
                }
                Outgoing::Error(error) => to_app.emit(ERROR_EVENT, error),
                // A like on the device played on: the same here.
                Outgoing::Liked(like) => {
                    let app = to_app.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        if let Err(e) = super::likes::apply(&app, &like) {
                            eprintln!("melue: a like from the other device: {e}");
                        }
                    });
                    Ok(())
                }
            };
        },
        move |how| closed(&closing, serial, how),
    )
    .await
    .map_err(|e| format!("{}: {e}", device.name))?;
    Ok((connected, link))
}

/// Connection `serial` ended: lost, it's tried again (`remote-reconnecting`); ended there, the connection's over.
/// Only when it's still the connection there is (not one replaced, or closed here).
fn closed(app: &AppHandle, serial: u64, how: Closed) {
    let remote = app.state::<Remote>();
    let mut current = remote.0.lock().unwrap();
    let Some(now) = current.as_mut().filter(|c| c.serial == serial) else { return };
    match how {
        Closed::Lost(_) => {
            now.link = None;
            let id = now.id.clone();
            drop(current);
            let _ = app.emit(RECONNECTING_EVENT, true);
            tauri::async_runtime::spawn(reconnect(app.clone(), id, serial));
        }
        Closed::Ended(why) => {
            drop(current);
            end(app, Some(why));
        }
        Closed::Disconnected => {
            drop(current);
            end(app, None);
        }
    }
}

/// The connection's over: Continue is back (not offering what that device saves as it stops, for a minute), and the
/// app's told (`remote-closed`, with why).
fn end(app: &AppHandle, why: Option<String>) {
    let remote = app.state::<Remote>();
    if let Some(Current { id, .. }) = remote.0.lock().unwrap().take() {
        handoff::connection_ended();
        handoff::ignore(&id, crate::library::now_ms() + 60_000);
    }
    remote.1.lock().unwrap().take();
    remote.2.lock().unwrap().take();
    media::hide(app);
    let _ = app.emit(CLOSED_EVENT, why);
}

/// Whether this device plays on another now (connected, or reconnecting).
pub fn connected(app: &AppHandle) -> bool {
    app.state::<Remote>().0.lock().unwrap().is_some()
}

/// Connection `serial` to device `id` was lost (a network change, the other device's network): tried again every few
/// seconds for a minute, while it's still the connection there is (not disconnected or replaced meanwhile). The other
/// device goes on playing; Melue closed there (offline), or a minute without reaching it, ends the connection.
async fn reconnect(app: AppHandle, id: String, serial: u64) {
    let still = |app: &AppHandle| app.state::<Remote>().0.lock().unwrap().as_ref().is_some_and(|c| c.serial == serial);
    let looked_up = |app: &AppHandle, id: &str| {
        let (app, id) = (app.clone(), id.to_string());
        tauri::async_runtime::spawn_blocking(move || presence::device(&app, &id))
    };
    let name = looked_up(&app, &id).await.ok().and_then(Result::ok).flatten().map_or_else(|| "the other device".to_string(), |d| d.name);
    let until = tokio::time::Instant::now() + RECONNECT_FOR;
    loop {
        tokio::time::sleep(RETRY_EVERY).await;
        if !still(&app) {
            return;
        }
        // Melue closed there (it said so as it went), or the device is gone: no use trying.
        if let Ok(Ok(None) | Ok(Some(Device { online: false, .. }))) = looked_up(&app, &id).await {
            if still(&app) {
                end(&app, Some(format!("{name} isn’t online anymore")));
            }
            return;
        }
        if let Ok((_, link)) = open(&app, &id, serial).await {
            let remote = app.state::<Remote>();
            let mut current = remote.0.lock().unwrap();
            if let Some(now) = current.as_mut().filter(|c| c.serial == serial) {
                now.link = Some(link);
                drop(current);
                let _ = app.emit(RECONNECTING_EVENT, false);
            }
            return;
        }
        if tokio::time::Instant::now() >= until {
            if still(&app) {
                end(&app, Some(format!("couldn’t reach {name} for a minute")));
            }
            return;
        }
    }
}

/// Connect device: plays what plays here on device `id` (from where it is) and pauses here. Paused here while it
/// plays there: just connected (a quiet join: what plays there goes on), to control it from here.
#[tauri::command]
pub async fn remote_play_on(app: AppHandle, id: String) -> Result<Connected, String> {
    let connected = remote_connect(app.clone(), id).await?;
    // Its first state (it says it at once on connecting): playing there and not here, this device just joins in
    // (what plays there goes on; this one becomes its remote), nothing is sent over.
    let mut there_playing = None;
    for _ in 0..20 {
        if let Some((_, state)) = app.state::<Remote>().1.lock().unwrap().as_ref() {
            there_playing = Some(state.playing);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let here_playing = app.native_audio().get_state().map(|s| s.is_playing).unwrap_or(false);
    if there_playing == Some(true) && !here_playing {
        return Ok(connected);
    }
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
/// that aren't on the share can't play there: left out (the one to start on: refused, saying so).
#[tauri::command(async)]
pub fn remote_load(app: AppHandle, ids: Vec<i64>, index: usize, playing: bool) -> Result<(), String> {
    let paths = share_paths(&app)?;
    let start = *ids.get(index).ok_or("no such song")?;
    if !paths.contains_key(&start) {
        return Err(only_here(&app, start));
    }
    let index = ids[..index].iter().filter(|id| paths.contains_key(id)).count();
    let queue = ids.iter().filter_map(|id| paths.get(id).cloned()).collect();
    remote_command(app.clone(), Command::Load { queue, index, position: 0.0, playing })
}

/// Plays a list (a play source here: "playlist:12", "album:3"…) on the device connected to: its own copy of it, its
/// songs only there included, from song `start` (an id here; None: its first, or a random one with shuffle on
/// there). A list only on this device (a playlist kept here, a music folder) is refused, saying so.
#[tauri::command(async)]
pub fn remote_play_list(app: AppHandle, source: String, start: Option<i64>) -> Result<(), String> {
    let library = app.state::<Library>();
    let list = library.shared_list(&source).map_err(|e| capitalized(&e))?;
    let paths = share_paths(&app)?;
    let start = match start {
        Some(id) => Some(paths.get(&id).cloned().ok_or_else(|| only_here(&app, id))?),
        None => None,
    };
    let songs = library.queue_items(&source)?.iter().filter_map(|item| item.id.and_then(|id| paths.get(&id).cloned())).collect();
    remote_command(app.clone(), Command::PlayList { list, songs, start })
}

/// Songs (ids here), or a list (a play source), into Up next on the device connected to: right after the playing
/// song (`next`) or at the end. Songs only on this device can't go: left out (the number left out is returned); all
/// of them, or a list only on this device: refused, saying so.
#[tauri::command(async)]
pub fn remote_add(app: AppHandle, ids: Vec<i64>, source: Option<String>, next: bool) -> Result<usize, String> {
    let library = app.state::<Library>();
    let list = source.as_deref().map(|s| library.shared_list(s)).transpose().map_err(|e| capitalized(&e))?;
    let paths = share_paths(&app)?;
    let songs: Vec<String> = ids.iter().filter_map(|id| paths.get(id).cloned()).collect();
    if list.is_none() && songs.is_empty() {
        return Err(match ids.as_slice() {
            [id] => only_here(&app, *id),
            _ => format!("Those songs are only on this device, so they can’t play on {}", there(&app)),
        });
    }
    remote_command(app.clone(), Command::Add { songs, list, next })?;
    Ok(if source.is_some() { 0 } else { ids.len() - ids.iter().filter(|id| paths.contains_key(id)).count() })
}

/// The songs here that are on the share: their paths there, by id.
fn share_paths(app: &AppHandle) -> Result<std::collections::HashMap<i64, String>, String> {
    let library = app.state::<Library>();
    let folder = library.synced_folder()?.ok_or("no share is set up")?;
    Ok(library.synced_songs(&folder)?.into_iter().collect())
}

/// "“Valerie” is only on this device, so it can't play on TROLLGANG".
fn only_here(app: &AppHandle, id: i64) -> String {
    let title = app.state::<Library>().queue_items_for(&[id]).ok().and_then(|items| items.into_iter().next()?.title);
    let song = title.map_or_else(|| "That song".to_string(), |t| format!("“{t}”"));
    format!("{song} is only on this device, so it can’t play on {}", there(app))
}

/// The device connected to, by name.
fn there(app: &AppHandle) -> String {
    let remote = app.state::<Remote>();
    let name = remote.1.lock().unwrap().as_ref().map(|(c, _)| c.name.clone());
    name.filter(|n| !n.is_empty()).unwrap_or_else(|| "the other device".into())
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// Whether this device plays on another now: which, and that one's state as it last said (None: playing here).
#[tauri::command]
pub fn remote_status(app: AppHandle) -> Option<(Connected, Option<RemoteState>)> {
    let remote = app.state::<Remote>();
    let connected = remote.0.lock().unwrap().as_ref().map(|c| c.id.clone())?;
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
    let current = current.as_ref().ok_or("not connected to another device")?;
    current.link.as_ref().ok_or("reconnecting to it: a moment")?.send(command)
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
    if let Some(Current { id, .. }) = remote.0.lock().unwrap().take() {
        handoff::connection_ended();
        // A minute more: what that device saves as it stops isn't offered back here.
        handoff::ignore(&id, crate::library::now_ms() + 60_000);
    }
    remote.1.lock().unwrap().take();
    remote.2.lock().unwrap().take();
    media::hide(&app);
}
