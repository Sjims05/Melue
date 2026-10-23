//! Likes over the connection: while one device plays on another, a song liked or unliked on either is liked or
//! unliked on the other at once (the share brings it later too; the newest change wins, so they agree). Songs go by
//! their path on the share. A like that came over the connection isn't sent on again.

use tauri::{AppHandle, Manager};

use super::control::{self, Command, Like, Outgoing};
use crate::library::changes::{self, LibraryChange};
use crate::library::{now_ms, Library};

/// Song `id` was liked or unliked here (by someone here): the device played on, and the devices playing on this
/// one, hear of it.
pub fn liked_here(app: &AppHandle, id: i64, liked: bool) {
    let controlling = super::link::connected(app);
    if !controlling && control::controllers().is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = path_of(&app, id) else { return };
        let like = Like { path, liked, at: now_ms() };
        if controlling {
            let _ = super::link::remote_command(app.clone(), Command::Like(like.clone()));
        }
        control::tell_controllers(Outgoing::Liked(like));
    });
}

/// A like from the other device: the same here (when the song's here), and the app hears of it as of any like.
pub fn apply(app: &AppHandle, like: &Like) -> Result<(), String> {
    let id = super::link::remote_song_ids(app.clone(), vec![Some(like.path.clone())])?
        .into_iter()
        .next()
        .flatten()
        .ok_or("that song isn’t on this device")?;
    app.state::<Library>().set_liked_at(id, like.liked, like.at)?;
    changes::emit(app, LibraryChange::liked(id));
    Ok(())
}

/// Song `id`'s path on the share (None: it isn't synced).
fn path_of(app: &AppHandle, id: i64) -> Option<String> {
    let library = app.state::<Library>();
    let folder = library.synced_folder().ok()??;
    library.synced_songs(&folder).ok()?.into_iter().find(|(song, _)| *song == id).map(|(_, path)| path)
}
