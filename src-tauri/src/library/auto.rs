//! The Android Auto library: built from the database and sent to the player, which saves it on the phone so
//! Android Auto can browse and play it with Melue closed. Sent at start-up and after library changes, once
//! things have been quiet for a moment (a burst of changes sends the library once, with all of them).
//!
//! Tabs: Recently played (the player's own list of lists played, which updates by itself), Playlists (the auto
//! playlists first, see auto_playlists.rs; empty and hidden ones left out), Albums, Artists (each one list of their songs, album by album). Folder ids are the list ids
//! ("album:12"), so a list played in the car is one the queue can follow (follow.rs). No "All songs" tab
//! (it's an auto playlist): search and voice find single songs. Only songs that can be played are in it. No section headers: in the
//! car a plain ordered list is easier to follow (no lines shifting).

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_native_audio::{Library as AutoLibrary, LibraryChild, LibraryFolder, LibraryItem, NativeAudioExt};

use super::covers::{self, LARGE};
use super::{Library, Result};

/// The player's tracked list of lists played (configured by the UI at start-up, see playback.ts).
pub const RECENT_LISTS: &str = "recentLists";
const EMPTY_MESSAGE: &str = "Add music in Melue on your phone";
/// How long things have to be quiet after a change before the library is sent.
const SETTLE: Duration = Duration::from_secs(2);

fn plural(n: i64, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// A list: tapping it opens it, and a song in it plays the whole list from that song (so the queue plays the
/// list, and follows it).
fn folder(id: String, title: String, subtitle: Option<String>, children: Vec<LibraryChild>) -> LibraryFolder {
    LibraryFolder { id, title, subtitle, playable: true, open_on_tap: true, style: Some("list".into()), children, ..Default::default() }
}

fn tab(id: &str, title: &str, style: &str, children: Vec<LibraryChild>) -> LibraryFolder {
    LibraryFolder { id: id.into(), title: title.into(), style: Some(style.into()), children, ..Default::default() }
}

impl Library {
    /// A list's songs, as folder children.
    fn auto_children(&self, source: &str) -> Result<Vec<LibraryChild>> {
        Ok(self.queue_items(source)?.into_iter().filter_map(|item| item.id).map(LibraryChild::Item).collect())
    }

    /// The library for Android Auto.
    pub fn auto_library(&self) -> Result<AutoLibrary> {
        let songs = self.queue_items("songs")?;
        if songs.is_empty() {
            return Ok(AutoLibrary { empty_message: Some(EMPTY_MESSAGE.into()), ..Default::default() });
        }
        let items = songs
            .into_iter()
            .filter_map(|song| {
                Some(LibraryItem {
                    id: song.id?,
                    src: song.src,
                    title: song.title,
                    artist: song.artist,
                    group: None,
                    album: song.album,
                    artwork_url: song.artwork_url,
                    progress: None,
                })
            })
            .collect();

        let mut playlists = Vec::new();
        for list in self.auto_playlists()?.into_iter().filter(|l| !l.hidden) {
            let children = self.auto_children(&list.source)?;
            if children.is_empty() {
                continue;
            }
            let subtitle = plural(children.len() as i64, "song");
            playlists.push(LibraryChild::Folder(folder(list.source, list.name, Some(subtitle), children)));
        }
        for playlist in self.playlists()? {
            let children = self.auto_children(&format!("playlist:{}", playlist.id))?;
            if children.is_empty() {
                continue;
            }
            let subtitle = format!("{} · {} min", plural(playlist.available, "song"), (playlist.duration_ms + 30_000) / 60_000);
            playlists.push(LibraryChild::Folder(folder(format!("playlist:{}", playlist.id), playlist.name, Some(subtitle), children)));
        }

        let mut albums = Vec::new();
        for album in self.albums(0, u32::MAX, Default::default())? {
            let source = format!("album:{}", album.id);
            let children = self.auto_children(&source)?;
            let artist = album.artist.unwrap_or_else(|| "Various artists".into());
            let subtitle = match album.year {
                Some(year) => format!("{artist} · {year}"),
                None => artist,
            };
            albums.push(LibraryChild::Folder(LibraryFolder {
                artwork_url: album.cover.map(|c| self.covers_dir().join(covers::file_name(&c, LARGE)).to_string_lossy().into_owned()),
                ..folder(source, album.title, Some(subtitle), children)
            }));
        }

        let mut artists = Vec::new();
        for artist in self.artists(0, u32::MAX, Default::default())? {
            let source = format!("artist:{}", artist.id);
            let children = self.auto_children(&source)?;
            if children.is_empty() {
                continue; // only an album artist of songs credited to others
            }
            let subtitle = plural(children.len() as i64, "song");
            artists.push(LibraryChild::Folder(folder(source, artist.name, Some(subtitle), children)));
        }

        let recent = LibraryFolder {
            id: "recent".into(),
            title: "Recently played".into(),
            style: Some("grid".into()),
            tracked_list: Some(RECENT_LISTS.into()),
            ..Default::default()
        };
        Ok(AutoLibrary {
            items,
            root: vec![recent, tab("playlists", "Playlists", "list", playlists), tab("albums", "Albums", "grid", albums), tab("artists", "Artists", "list", artists)],
            search: Some(true),
            empty_message: Some(EMPTY_MESSAGE.into()),
            ..Default::default()
        })
    }
}

/// Sending the library: a background thread that sends it once things settle.
#[derive(Default)]
pub struct AutoSender {
    soon: Mutex<Option<Sender<()>>>,
}

/// Android: sends the library now, and after each burst of changes (see [`send_soon`]). Elsewhere there's
/// no Android Auto: nothing to do.
pub fn start(app: &AppHandle) {
    if !cfg!(target_os = "android") {
        return;
    }
    let (soon, wake) = mpsc::channel::<()>();
    *app.state::<AutoSender>().soon.lock().unwrap() = Some(soon);
    let app = app.clone();
    std::thread::Builder::new()
        .name("android-auto-library".into())
        .spawn(move || {
            send(&app);
            while wake.recv().is_ok() {
                loop {
                    match wake.recv_timeout(SETTLE) {
                        Ok(()) => continue,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                send(&app);
            }
        })
        .expect("the Android Auto library thread");
}

/// The library changed: send it once things have been quiet for a moment.
pub fn send_soon(app: &AppHandle) {
    if let Some(sender) = app.try_state::<AutoSender>() {
        if let Some(soon) = sender.soon.lock().unwrap().as_ref() {
            let _ = soon.send(());
        }
    }
}

fn send(app: &AppHandle) {
    let built = app.state::<Library>().auto_library();
    match built.and_then(|library| app.native_audio().set_library(&library)) {
        Ok(()) => {}
        Err(e) => eprintln!("melue: can't send the Android Auto library: {e}"),
    }
}
