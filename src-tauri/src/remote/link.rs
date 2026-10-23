//! This device controlling another (the app's side): one connection at a time. Its state comes to the app as
//! `remote-state` events, a command that didn't work there as `remote-error`, and the end of the connection as
//! `remote-closed` (with why, unless this device closed it).

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::control::{self, Command, Link, Outgoing};
use super::{key, presence, server};
use crate::library::Library;
use crate::sync::handoff;

pub const STATE_EVENT: &str = "remote-state";
pub const ERROR_EVENT: &str = "remote-error";
pub const CLOSED_EVENT: &str = "remote-closed";

/// The connection, and to which device.
#[derive(Default)]
pub struct Remote(Mutex<Option<(String, Link)>>);

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
    let (to_app, closing) = (app.clone(), app.clone());
    let closed_id = id.clone();
    let link = control::connect(
        &reached.address,
        port,
        &key,
        &me,
        move |message| {
            let _ = match message {
                Outgoing::State(state) => to_app.emit(STATE_EVENT, state),
                Outgoing::Error(error) => to_app.emit(ERROR_EVENT, error),
            };
        },
        move |why| {
            // Only when it's still the connection there is (not one replaced or closed here).
            let remote = closing.state::<Remote>();
            let mut current = remote.0.lock().unwrap();
            if current.as_ref().is_some_and(|(id, _)| *id == closed_id) {
                *current = None;
                let _ = closing.emit(CLOSED_EVENT, why);
            }
        },
    )
    .await
    .map_err(|e| format!("{}: {e}", device.name))?;
    *app.state::<Remote>().0.lock().unwrap() = Some((id.clone(), link));
    Ok(Connected { id, name: device.name, address: reached.address })
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

/// Closes the connection (the other device goes on as it is).
#[tauri::command]
pub fn remote_disconnect(app: AppHandle) {
    // Dropping the link closes it.
    app.state::<Remote>().0.lock().unwrap().take();
}
