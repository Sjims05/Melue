//! The library's commands for the UI. They run off the main thread (`async`), so a slow query never
//! freezes the window.

use super::auto_playlists::AutoPlaylist;
use serde::Serialize;
use tauri::{AppHandle, State};

use super::scan::ScanSummary;
use super::changes::{self, tracking, ChangeReason, LibraryChange};
use super::follow::QueueFollow;
use super::playback::{Outcome, PlaybackEvent};
use super::player_settings::PlayerSettings;
use super::playlists::{Playlist, PlaylistEntry};
use super::scanner::AppFiles;
use super::{android, scanner, Album, AlbumPage, AlbumSort, Artist, ArtistPage, ArtistSort, Folder, FolderKind, Library, Result, Track, TrackSort};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub schema_version: i64,
    pub folders: i64,
    /// Tracks whose files are there (not missing).
    pub tracks: i64,
    /// Albums and artists with songs that are there.
    pub albums: i64,
    pub artists: i64,
    pub playlists: i64,
}

#[tauri::command(async)]
pub fn library_status(library: State<'_, Library>) -> Result<LibraryStatus> {
    library.status()
}

/// A song's lyrics (Now playing): from a .lrc next to it, its tags, or found online; null when it has none. A song
/// without any that wasn't looked up online yet is looked up now, when Melue may (Wi-Fi, the setting on).
#[tauri::command(async)]
pub fn library_lyrics(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<Option<super::lyrics::Lyrics>> {
    let open = |job: &super::tags::TagJob, path: &str| super::scanner::open_file(&app, job, path);
    if let Some(lyrics) = library.lyrics(id, &open)? {
        return Ok(Some(lyrics));
    }
    if library.online_lyrics(id)?.is_none() && super::online_lyrics::may_look_up(&app) {
        if let Err(e) = super::online_lyrics::look_up_now(&app, id) {
            eprintln!("melue: lyrics: LRCLIB can't be reached now: {e}");
        }
        return library.lyrics(id, &open);
    }
    Ok(None)
}

/// "Try another version": the song's lyrics from LRCLIB's next entry, read again (null: there's no other).
#[tauri::command(async)]
pub fn library_lyrics_other_version(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<Option<super::lyrics::Lyrics>> {
    if !super::online_lyrics::try_other_version(&app, id)? {
        return Ok(None);
    }
    library.lyrics(id, &|job, path| super::scanner::open_file(&app, job, path))
}

/// A song's lyrics timing set by hand: moved earlier or later (ms; positive: later), and lines tapped.
#[tauri::command(async)]
pub fn library_set_lyrics_timing(
    app: AppHandle,
    library: State<'_, Library>,
    id: i64,
    offset_ms: i64,
    anchors: Vec<super::lyrics::Anchor>,
) -> Result<()> {
    library.set_lyrics_timing(id, &super::lyrics::Timing { offset_ms, anchors }, super::now_ms())?;
    // To the other devices with the library data.
    crate::sync::data::soon(&app);
    Ok(())
}

/// Music folders that are the share, inside it, or look like it (folders.rs share_folders): their ids and why.
#[tauri::command(async)]
pub fn library_share_folders(library: State<'_, Library>) -> Result<Vec<(i64, &'static str)>> {
    library.share_folders()
}

/// The songs whose files can't be found (Library: Show).
#[tauri::command(async)]
pub fn library_missing_songs(library: State<'_, Library>) -> Result<Vec<super::tracks::MissingSong>> {
    library.missing_songs()
}

/// Forgets songs that can't be found (`ids`; none given: all): see Library::forget_missing.
#[tauri::command(async)]
pub fn library_forget_missing(app: AppHandle, library: State<'_, Library>, ids: Option<Vec<i64>>) -> Result<usize> {
    let gone = library.forget_missing(ids.as_deref())?;
    if gone > 0 {
        changes::emit(&app, LibraryChange::sync());
    }
    Ok(gone)
}

/// An app setting saved in the library (null when it was never set).
#[tauri::command(async)]
pub fn library_setting(library: State<'_, Library>, key: String) -> Result<Option<String>> {
    library.setting(&key)
}

// ---- music folders

#[tauri::command(async)]
pub fn library_folders(library: State<'_, Library>) -> Result<Vec<Folder>> {
    library.folders()
}

/// Adds a music folder: a path; all music on the phone (asks for the permission); or a folder chosen
/// with Android's storage picker (keeps access to it).
#[tauri::command(async)]
pub fn library_add_folder(
    app: AppHandle,
    library: State<'_, Library>,
    kind: FolderKind,
    location: String,
    name: Option<String>,
) -> Result<Folder> {
    match kind {
        FolderKind::Path => {}
        FolderKind::AndroidMedia => android::ensure_music_permission(&app)?,
        FolderKind::AndroidTree => android::keep_folder_access(&app, &location)?,
    }
    let added = tracking(&app, ChangeReason::Folders, true, || library.add_folder(kind, &location, name.as_deref()));
    if added.is_err() && kind == FolderKind::AndroidTree {
        android::release_folder_access(&app, &location);
    }
    added
}

/// Removes a folder and its tracks from the library (the files themselves stay).
#[tauri::command(async)]
pub fn library_remove_folder(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<()> {
    let folder = library.folder(id)?.ok_or_else(|| format!("no folder {id}"))?;
    tracking(&app, ChangeReason::Folders, true, || library.remove_folder(id))?;
    if folder.kind == FolderKind::AndroidTree {
        android::release_folder_access(&app, &folder.location);
    }
    // Tidies up the covers only its songs used.
    scanner::scan_soon(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn library_set_folder_enabled(app: AppHandle, library: State<'_, Library>, id: i64, enabled: bool) -> Result<Folder> {
    tracking(&app, ChangeReason::Folders, true, || library.set_folder_enabled(id, enabled))
}

#[tauri::command(async)]
pub fn library_rename_folder(app: AppHandle, library: State<'_, Library>, id: i64, name: String) -> Result<Folder> {
    tracking(&app, ChangeReason::Folders, true, || library.rename_folder(id, &name))
}

/// Scans every switched-on folder (the refresh button) and returns what changed; null when a scan was
/// already running (it goes once more instead). Progress also comes as `library-scan` events.
#[tauri::command(async)]
pub fn library_scan(app: AppHandle) -> Result<Option<ScanSummary>> {
    scanner::scan_all(&app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicAccess {
    /// "granted", "denied", "prompt" (not asked yet) or "prompt-with-rationale" (denied once).
    pub permission: String,
    /// Android: how many songs the phone's music index has, once allowed.
    pub songs: Option<u64>,
}

/// Android: whether Melue may read the music on the phone (asking first when `request`). Desktop: granted.
#[tauri::command(async)]
pub fn library_music_access(app: AppHandle, request: bool) -> Result<MusicAccess> {
    let (permission, songs) = android::music_access(&app, request)?;
    Ok(MusicAccess { permission, songs })
}

#[tauri::command(async)]
pub fn library_set_setting(app: AppHandle, library: State<'_, Library>, key: String, value: String) -> Result<()> {
    let sync_before = library.sync_available()?;
    library.set_setting(&key, &value)?;
    if library.sync_available()? != sync_before {
        changes::emit(&app, LibraryChange::sync());
    }
    // Finding lyrics online switched on: a round now.
    if key == super::online_lyrics::SETTING {
        super::online_lyrics::soon(&app);
    }
    Ok(())
}

/// Whether sync is available: a share is set up (the UI shows sync buttons only then).
#[tauri::command(async)]
pub fn library_sync_available(library: State<'_, Library>) -> Result<bool> {
    library.sync_available()
}

// ---- songs

/// Songs that are there, by title (`limit` from `offset`).
#[tauri::command(async)]
pub fn library_tracks(library: State<'_, Library>, offset: u32, limit: u32, sort: Option<TrackSort>) -> Result<Vec<Track>> {
    library.tracks(offset, limit, sort.unwrap_or_default())
}

/// Albums with songs that are there, by title.
#[tauri::command(async)]
pub fn library_albums(library: State<'_, Library>, offset: u32, limit: u32, sort: Option<AlbumSort>) -> Result<Vec<Album>> {
    library.albums(offset, limit, sort.unwrap_or_default())
}

/// Artists with songs that are there, by name (sort name: "The" ignored).
#[tauri::command(async)]
pub fn library_artists(library: State<'_, Library>, offset: u32, limit: u32, sort: Option<ArtistSort>) -> Result<Vec<Artist>> {
    library.artists(offset, limit, sort.unwrap_or_default())
}

/// Songs ready to play, in the order given (songs not there left out): Play next and Add to queue.
#[tauri::command(async)]
pub fn library_queue_items_for(library: State<'_, Library>, ids: Vec<i64>) -> Result<Vec<tauri_plugin_native_audio::QueueItem>> {
    library.queue_items_for(&ids)
}

/// Playlist files let go of (unlinked, or their playlist deleted): Settings' "Follow again".
#[tauri::command(async)]
pub fn library_let_go_files(library: State<'_, Library>) -> Result<Vec<super::linked::LetGoFile>> {
    library.let_go_files()
}

/// Takes a let-go playlist file whose file is gone off the list.
#[tauri::command(async)]
pub fn library_forget_let_go_file(app: AppHandle, library: State<'_, Library>, folder_id: i64, path: String) -> Result<()> {
    library.forget_let_go_file(folder_id, &path)?;
    changes::emit(&app, LibraryChange::playlist(0, Vec::new()));
    Ok(())
}

/// Search: songs (the first `limit`, and how many in all), albums, artists and playlists matching `query`.
#[tauri::command(async)]
pub fn library_search(library: State<'_, Library>, query: String, limit: usize) -> Result<super::search::SearchResults> {
    library.search(&query, limit)
}

/// Gives a playlist its own cover (`picture`: a JPEG, PNG or WebP, cropped square).
#[tauri::command(async)]
pub fn library_set_playlist_cover(app: AppHandle, library: State<'_, Library>, id: i64, picture: Vec<u8>) -> Result<Playlist> {
    let playlist = library.set_playlist_cover(id, &picture)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    Ok(playlist)
}

/// Back to a playlist's default cover.
#[tauri::command(async)]
pub fn library_clear_playlist_cover(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<Playlist> {
    let playlist = library.clear_playlist_cover(id)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    Ok(playlist)
}

/// Adds music files (dropped on Melue, or picked) to a music folder on this computer: copied into
/// `Artist/Album/`, then scanned in; `playlist`: they go into it too, once the scan has found them.
#[tauri::command(async)]
pub fn library_add_songs_to_folder(
    app: AppHandle,
    library: State<'_, Library>,
    folder: i64,
    files: Vec<String>,
    playlist: Option<i64>,
) -> Result<super::adding::AddResult> {
    let result = library.copy_songs_into(folder, &files)?;
    if let Some(playlist) = playlist {
        library.wait_for_songs(folder, &result.placed, playlist)?;
    }
    scanner::scan_soon(&app);
    Ok(result)
}

/// A picture dropped on Melue (a playlist's Change cover): its bytes. Only JPEG, PNG and WebP files up to 30 MB, so
/// this can't read other files.
#[tauri::command(async)]
pub fn library_read_picture(path: String) -> Result<Vec<u8>> {
    const PICTURES: [&str; 4] = ["jpg", "jpeg", "png", "webp"];
    let path = std::path::Path::new(&path);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
    if !PICTURES.contains(&extension.as_str()) {
        return Err("that isn't a picture Melue can read (JPEG, PNG or WebP)".into());
    }
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 30 * 1024 * 1024 {
        return Err("that picture is too big (over 30 MB)".into());
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

/// The playlists a song is in (Add to playlist's ✓).
#[tauri::command(async)]
pub fn library_playlists_with_track(library: State<'_, Library>, id: i64) -> Result<Vec<i64>> {
    library.playlists_with_track(id)
}

/// Exports a list's songs as an .m3u8 playlist file at `path`. Resolves with how many songs it holds.
#[tauri::command(async)]
pub fn library_export_playlist(library: State<'_, Library>, source: String, path: String) -> Result<usize> {
    library.export_m3u(&source, std::path::Path::new(&path))
}

/// Stops or starts following a linked playlist's file; following again scans, to show how it differs from the file.
#[tauri::command(async)]
pub fn library_set_playlist_following(app: AppHandle, library: State<'_, Library>, id: i64, following: bool) -> Result<()> {
    library.set_playlist_following(id, following)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    if following {
        scanner::scan_all(&app)?;
    }
    Ok(())
}

/// Linked playlists deleted with the user's changes in the last 30 days (Recently deleted, this device).
#[tauri::command(async)]
pub fn library_deleted_linked_playlists(library: State<'_, Library>) -> Result<Vec<super::linked::DeletedLinkedPlaylist>> {
    library.deleted_linked_playlists()
}

/// Restores a deleted linked playlist as it was, and scans so it's in step with its file. Resolves with its id.
#[tauri::command(async)]
pub fn library_restore_linked_playlist(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<i64> {
    let restored = library.restore_linked_playlist(id)?;
    changes::emit(&app, LibraryChange::playlist(restored, Vec::new()));
    scanner::scan_all(&app)?;
    Ok(restored)
}

/// Follows a let-go playlist file again, and scans so it's linked as a playlist now.
#[tauri::command(async)]
pub fn library_follow_file_again(app: AppHandle, library: State<'_, Library>, folder_id: i64, path: String) -> Result<()> {
    library.follow_file_again(folder_id, &path)?;
    scanner::scan_all(&app)?;
    Ok(())
}

/// An album's page: the album and its songs (null: no such album).
#[tauri::command(async)]
pub fn library_album_page(library: State<'_, Library>, id: i64) -> Result<Option<AlbumPage>> {
    library.album_page(id)
}

/// An artist's page: the artist, their albums, their songs and the songs they're featured on (null: no such artist).
#[tauri::command(async)]
pub fn library_artist_page(library: State<'_, Library>, id: i64) -> Result<Option<ArtistPage>> {
    library.artist_page(id)
}

/// A Made for you list's songs, in its order ("songs", "liked", "top:<period>").
#[tauri::command(async)]
pub fn library_auto_playlist_tracks(library: State<'_, Library>, source: String) -> Result<Vec<Track>> {
    library.auto_playlist_tracks(&source)
}

/// A music folder's songs, by where they are in it.
#[tauri::command(async)]
pub fn library_folder_tracks(library: State<'_, Library>, id: i64) -> Result<Vec<Track>> {
    library.folder_tracks(id)
}

/// Desktop: opens the song's folder in the file manager with the file selected ("Show in folder").
#[tauri::command(async)]
pub fn library_show_in_folder(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<()> {
    let path = library.track_file(id)?.ok_or("this song isn't a file on this device's disk")?;
    #[cfg(desktop)]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener().reveal_item_in_dir(&path).map_err(|e| format!("can't show {}: {e}", path.display()))
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, path);
        Err("Show in folder is for desktop".into())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCovers {
    /// Whether all music on the phone uses the cover images next to the songs (cover.jpg, folder.jpg).
    pub enabled: bool,
    /// The photos permission they need: "granted", "denied", "prompt"... Desktop: granted.
    pub permission: String,
}

/// Android, all music on the phone: whether cover images next to the songs are used (off unless switched on).
#[tauri::command(async)]
pub fn library_folder_covers(app: AppHandle, library: State<'_, Library>) -> Result<FolderCovers> {
    let enabled = library.setting(android::FOLDER_COVERS_SETTING)?.as_deref() == Some("1");
    Ok(FolderCovers { enabled, permission: android::photos_access(&app, false)? })
}

/// Switches cover images next to the songs on or off, then rescans. Switching on asks for the photos
/// permission, and is refused (stays off) without it.
#[tauri::command(async)]
pub fn library_set_folder_covers(app: AppHandle, library: State<'_, Library>, enabled: bool) -> Result<FolderCovers> {
    if enabled && android::photos_access(&app, true)? != "granted" {
        return Err("Melue needs permission to read photos for the cover images next to the songs".into());
    }
    library.set_setting(android::FOLDER_COVERS_SETTING, if enabled { "1" } else { "0" })?;
    scanner::scan_soon(&app);
    library_folder_covers(app, library)
}

// ---- playlists (each change is told to the UI as a library change)

/// Every playlist, by name.
#[tauri::command(async)]
pub fn library_playlists(library: State<'_, Library>) -> Result<Vec<Playlist>> {
    library.playlists()
}

#[tauri::command(async)]
pub fn library_playlist(library: State<'_, Library>, id: i64) -> Result<Playlist> {
    library.playlist(id)
}

/// A playlist's songs in order (unavailable ones too: shown greyed).
#[tauri::command(async)]
pub fn library_playlist_entries(library: State<'_, Library>, id: i64) -> Result<Vec<PlaylistEntry>> {
    library.playlist_entries(id)
}

#[tauri::command(async)]
pub fn library_create_playlist(app: AppHandle, library: State<'_, Library>, name: String) -> Result<Playlist> {
    let playlist = library.create_playlist(&name)?;
    changes::emit(&app, LibraryChange::playlist(playlist.id, Vec::new()));
    Ok(playlist)
}

#[tauri::command(async)]
pub fn library_rename_playlist(app: AppHandle, library: State<'_, Library>, id: i64, name: String) -> Result<Playlist> {
    let playlist = library.rename_playlist(id, &name)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    Ok(playlist)
}

#[tauri::command(async)]
pub fn library_delete_playlist(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<()> {
    let tracks = library.playlist_entries(id)?.into_iter().filter_map(|e| e.track.map(|t| t.id)).collect();
    library.delete_playlist(id)?;
    changes::emit(&app, LibraryChange::playlist(id, tracks));
    Ok(())
}

/// Adds songs at `position` (the end when left out); returns the new entries' ids.
#[tauri::command(async)]
pub fn library_add_to_playlist(
    app: AppHandle,
    library: State<'_, Library>,
    id: i64,
    tracks: Vec<i64>,
    position: Option<usize>,
) -> Result<Vec<i64>> {
    let added = library.add_to_playlist(id, &tracks, position)?;
    changes::emit(&app, LibraryChange::playlist(id, tracks));
    write_back(&app, &library, id)?;
    Ok(added)
}

/// Removes entries (by entry id: one copy of a song that's in twice).
#[tauri::command(async)]
pub fn library_remove_from_playlist(app: AppHandle, library: State<'_, Library>, id: i64, entries: Vec<i64>) -> Result<()> {
    let removed = library.remove_from_playlist(id, &entries)?;
    changes::emit(&app, LibraryChange::playlist(id, removed));
    write_back(&app, &library, id)
}

/// Find song: `track` for the playlist's entry `entry` (one the automatic match left out).
#[tauri::command(async)]
pub fn library_choose_playlist_song(app: AppHandle, library: State<'_, Library>, id: i64, entry: i64, track: i64) -> Result<()> {
    let track = library.choose_playlist_song(id, entry, track)?;
    changes::emit(&app, LibraryChange::playlist(id, vec![track]));
    write_back(&app, &library, id)
}

#[tauri::command(async)]
pub fn library_move_in_playlist(app: AppHandle, library: State<'_, Library>, id: i64, entry: i64, position: usize) -> Result<()> {
    let track = library.move_in_playlist(id, entry, position)?;
    changes::emit(&app, LibraryChange::playlist(id, track.into_iter().collect()));
    write_back(&app, &library, id)
}

/// After an edit of a linked playlist with write-back on: Melue's version into the file. The edit is saved
/// either way; a failure to write is reported.
fn write_back(app: &AppHandle, library: &Library, id: i64) -> Result<()> {
    match library.write_back_if_on(id, &AppFiles(app)) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("saved in Melue, but the playlist file couldn't be written: {e}")),
    }
}

/// Takes a greyed song of a linked playlist (removed here, or suggested by the file) into Melue's version.
#[tauri::command(async)]
pub fn library_restore_playlist_entry(app: AppHandle, library: State<'_, Library>, id: i64, entry: i64) -> Result<()> {
    let track = library.restore_playlist_entry(id, entry)?;
    changes::emit(&app, LibraryChange::playlist(id, track.into_iter().collect()));
    write_back(&app, &library, id)
}

/// A linked playlist's settings (each left as it is when null): songs new in the file 'added' or
/// 'suggested', songs removed from it 'removed' or 'kept', and writing Melue's changes into the file
/// (switching it on writes the file right away).
#[tauri::command(async)]
pub fn library_set_playlist_options(
    app: AppHandle,
    library: State<'_, Library>,
    id: i64,
    new_in_file: Option<String>,
    removed_from_file: Option<String>,
    write_back: Option<bool>,
) -> Result<Playlist> {
    library.set_playlist_options(id, new_in_file.as_deref(), removed_from_file.as_deref(), write_back)?;
    let result = if write_back == Some(true) { self::write_back(&app, &library, id) } else { Ok(()) };
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    result?;
    library.playlist(id)
}

/// Turns a linked playlist into an app playlist (the file is left alone and no longer followed).
#[tauri::command(async)]
pub fn library_unlink_playlist(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<Playlist> {
    library.unlink_playlist(id)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    library.playlist(id)
}

// ---- playing (see playback.rs)

/// The queue for a list ("songs", "album:<id>", "artist:<id>", "playlist:<id>", "liked"), ready to play.
#[tauri::command(async)]
pub fn library_queue_items(library: State<'_, Library>, source: String) -> Result<Vec<tauri_plugin_native_audio::QueueItem>> {
    library.queue_items(&source)
}

/// Counts one of the player's playback events (once, however often it's handed over): "play", "skip" or
/// "neither" for a song being left, null otherwise.
#[tauri::command(async)]
pub fn library_record_playback(app: AppHandle, library: State<'_, Library>, event: PlaybackEvent) -> Result<Option<Outcome>> {
    let outcome = library.record_playback(&event)?;
    // A play can change the Top songs lists (Android Auto shows them), and goes to the other devices.
    if outcome == Some(Outcome::Play) {
        super::auto::send_soon(&app);
        crate::sync::data::soon(&app);
    }
    Ok(outcome)
}

/// Every auto playlist (All songs, Liked, Top songs, one per folder), in order, hidden ones too.
#[tauri::command(async)]
pub fn library_auto_playlists(library: State<'_, Library>) -> Result<Vec<AutoPlaylist>> {
    library.auto_playlists()
}

/// Hides an auto playlist or shows it again, and (Top songs) changes its size. Returns them all.
#[tauri::command(async)]
pub fn library_set_auto_playlist(
    app: AppHandle,
    library: State<'_, Library>,
    source: String,
    hidden: Option<bool>,
    size: Option<u32>,
) -> Result<Vec<AutoPlaylist>> {
    if let Some(size) = size {
        library.set_auto_playlist_size(&source, size)?;
    }
    if let Some(hidden) = hidden {
        library.set_auto_playlist_hidden(&source, hidden)?;
    }
    super::auto::send_soon(&app);
    library.auto_playlists()
}

#[tauri::command(async)]
pub fn library_set_liked(app: AppHandle, library: State<'_, Library>, id: i64, liked: bool) -> Result<()> {
    library.set_liked(id, liked)?;
    changes::emit(&app, LibraryChange::liked(id));
    // Playing on another device (or played from one): it's liked there at once too.
    crate::remote::likes::liked_here(&app, id, liked);
    Ok(())
}

/// Every liked song's id (for the player's like button).
#[tauri::command(async)]
pub fn library_liked_ids(library: State<'_, Library>) -> Result<Vec<i64>> {
    library.liked_ids()
}

#[tauri::command(async)]
pub fn library_recently_played(library: State<'_, Library>, limit: u32) -> Result<Vec<Track>> {
    library.recently_played(limit)
}

#[tauri::command(async)]
pub fn library_most_played(library: State<'_, Library>, limit: u32) -> Result<Vec<Track>> {
    library.most_played(limit)
}

/// How the queue follows library changes (on, and the player's updateQueue options).
#[tauri::command(async)]
pub fn library_queue_follow(library: State<'_, Library>) -> Result<QueueFollow> {
    library.queue_follow()
}

#[tauri::command(async)]
pub fn library_set_queue_follow(library: State<'_, Library>, settings: QueueFollow) -> Result<QueueFollow> {
    library.set_queue_follow(&settings)?;
    library.queue_follow()
}

/// The songs with these ids (the player's queue: lengths, likes, covers).
#[tauri::command(async)]
pub fn library_tracks_by_ids(library: State<'_, Library>, ids: Vec<i64>) -> Result<Vec<Track>> {
    library.tracks_by_ids(&ids)
}

/// How much space the covers take (Storage), in bytes.
#[tauri::command(async)]
pub fn library_covers_size(library: State<'_, Library>) -> u64 {
    library.covers_size()
}

/// Storage's Clean up: removes covers no song uses any more. Returns how many files went.
#[tauri::command(async)]
pub fn library_clean_covers(library: State<'_, Library>) -> Result<u32> {
    library.clean_covers()
}

/// The player's settings, as the settings page shows them (the frontend hands them to the player).
#[tauri::command(async)]
pub fn library_player_settings(library: State<'_, Library>) -> Result<PlayerSettings> {
    library.player_settings()
}

#[tauri::command(async)]
pub fn library_set_player_settings(library: State<'_, Library>, settings: PlayerSettings) -> Result<PlayerSettings> {
    library.set_player_settings(&settings)?;
    library.player_settings()
}

// ---- Spotify playlists (spotify.rs)

/// Adds a Spotify playlist from its link (a public one): a linked playlist following it. Its id.
#[tauri::command(async)]
pub fn library_add_spotify_playlist(app: AppHandle, library: State<'_, Library>, link: String) -> Result<i64> {
    let index = library.track_index()?;
    let id = library.add_spotify_playlist(&link, &index)?;
    changes::emit(&app, LibraryChange::playlists(ChangeReason::Playlists, vec![id]));
    super::spotify::update_covers_soon(&app);
    Ok(id)
}

/// A Spotify playlist shows its songs' covers (`songs`) or Spotify's picture.
#[tauri::command(async)]
pub fn library_set_songs_cover(app: AppHandle, library: State<'_, Library>, id: i64, songs: bool) -> Result<()> {
    library.set_songs_cover(id, songs)?;
    changes::emit(&app, LibraryChange::playlists(ChangeReason::Playlists, vec![id]));
    Ok(())
}

/// Reads a Spotify playlist (or, `id` None, all of them) again now. Whether any changed here.
#[tauri::command(async)]
pub fn library_update_spotify_playlists(app: AppHandle, library: State<'_, Library>, id: Option<i64>) -> Result<bool> {
    let index = library.track_index()?;
    let changed = match id {
        Some(id) => library.update_spotify_playlist(id, &index)?.then_some(vec![id]).unwrap_or_default(),
        None => library.update_spotify_playlists(&index)?,
    };
    if !changed.is_empty() {
        changes::emit(&app, LibraryChange::playlists(ChangeReason::Playlists, changed.clone()));
    }
    Ok(!changed.is_empty())
}

/// A long Spotify playlist's full list, from a CSV's text (a website's export).
#[tauri::command(async)]
pub fn library_import_spotify_csv(app: AppHandle, library: State<'_, Library>, id: i64, csv: String) -> Result<()> {
    let index = library.track_index()?;
    library.import_spotify_csv(id, &csv, &index)?;
    changes::emit(&app, LibraryChange::playlists(ChangeReason::Playlists, vec![id]));
    Ok(())
}

/// A long Spotify playlist's full list, from a CSV file picked on a computer (a .csv only).
#[tauri::command(async)]
pub fn library_import_spotify_csv_file(app: AppHandle, library: State<'_, Library>, id: i64, path: String) -> Result<()> {
    let file = std::path::Path::new(&path);
    if !file.extension().is_some_and(|x| x.eq_ignore_ascii_case("csv")) {
        return Err("that isn’t a CSV file".into());
    }
    let csv = std::fs::read_to_string(file).map_err(|e| format!("can't read {path}: {e}"))?;
    library_import_spotify_csv(app, library, id, csv)
}

/// Counts waits for a CSV started or stopped: a wait that sees it change stops.
static CSV_WAIT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Computers: waits (10 minutes at most) for a new CSV in the Downloads folder, the export just made, and imports it
/// into Spotify playlist `id`. Ok(true): imported; Ok(false): none came (or another wait started, or it was stopped).
#[tauri::command(async)]
pub fn library_wait_for_spotify_csv(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<bool> {
    use tauri::Manager;
    let attempt = CSV_WAIT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let downloads = app.path().download_dir().map_err(|e| e.to_string())?;
    let since = std::time::SystemTime::now();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(600);
    while std::time::Instant::now() < until && CSV_WAIT.load(std::sync::atomic::Ordering::SeqCst) == attempt {
        std::thread::sleep(std::time::Duration::from_millis(700));
        let newest = std::fs::read_dir(&downloads)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("csv")))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .filter(|(at, _)| *at > since)
            .max_by_key(|(at, _)| *at);
        if let Some((_, path)) = newest {
            // Fully written (a browser renames it into place when done; a moment more to be sure).
            std::thread::sleep(std::time::Duration::from_millis(500));
            let csv = std::fs::read_to_string(&path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
            library_import_spotify_csv(app.clone(), library, id, csv)?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Stops waiting for a CSV.
#[tauri::command]
pub fn library_stop_waiting_for_spotify_csv() {
    CSV_WAIT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}
