/**
 * Melue's music library, kept by the app's Rust side (src-tauri/src/library): the database, scanning
 * and tags. The UI uses these instead of calling the commands itself.
 */
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface LibraryStatus {
  /** The database's schema version (it upgrades itself when the app updates). */
  schemaVersion: number
  folders: number
  /** Tracks whose files are there (not missing). */
  tracks: number
  albums: number
  artists: number
  playlists: number
}

/**
 * Where music comes from: a folder on a disk ('path'), all music on an Android phone ('android_media'),
 * or a folder chosen with Android's storage picker ('android_tree', its location a content:// address).
 */
export type FolderKind = 'path' | 'android_media' | 'android_tree'

export interface MusicFolder {
  id: number
  kind: FolderKind
  location: string
  name: string
  /** Switched off: its songs are hidden but kept (with their likes and history). */
  enabled: boolean
  /** The folder a share syncs into. */
  synced: boolean
  addedAt: number
  lastScanAt: number | null
  /** Songs found in it. */
  tracks: number
  /** On another computer: removing songs from it deletes them for good (no Recycle Bin). */
  network: boolean
}

export interface MusicAccess {
  /** Whether Melue may read the music on the phone ('prompt': not asked yet). Always 'granted' on desktop. */
  permission: 'granted' | 'denied' | 'prompt' | 'prompt-with-rationale'
  /** Android: how many songs the phone's music index has, once allowed. */
  songs: number | null
}

/** What a scan changed. */
export interface ScanSummary {
  added: number
  changed: number
  /** Files that are gone (their songs are kept, hidden, until they're back). */
  missing: number
  /** Missing before, back now. */
  returned: number
  /** Found under another name or folder (they kept their likes, playlists and history). */
  moved: number
  unchanged: number
  /** Songs whose tags were read (new or changed files). */
  tagsRead: number
  /** Songs whose tags couldn't be read (they keep their file names as titles, and still play). */
  unreadable: number
}

/**
 * A scan's progress: it started, one folder is listed (or couldn't be read), tags are being read (the
 * second pass, for new and changed songs), it finished.
 */
export type ScanEvent =
  | { phase: 'started' }
  | { phase: 'folder'; folderId: number; name: string; summary: ScanSummary | null; error: string | null }
  | { phase: 'tags'; done: number; total: number }
  | { phase: 'finished'; summary: ScanSummary; changed: boolean }

/**
 * What changed in the library: sent once per change (a scan that found something, a folder added,
 * removed, switched on or off, or renamed).
 */
export interface LibraryChange {
  /** 'sync': whether sync is available changed (see sync.ts). */
  reason: 'scan' | 'folders' | 'playlists' | 'likes' | 'sync'
  /** Songs added, changed (tags, cover, file), gone (missing, removed, hidden with their folder) or back. */
  tracks: number[]
  /** The playable lists whose songs changed: 'songs' (all songs), 'album:<id>', 'artist:<id>', 'playlist:<id>', 'liked'. */
  sources: string[]
}

/**
 * A playlist made in Melue ('app'), or a playlist file (.m3u) in a music folder, linked into the library
 * ('linked'): Melue keeps its own version and shows how it differs from the file (see PlaylistEntry.state).
 */
export interface Playlist {
  id: number
  name: string
  kind: 'app' | 'linked'
  /** Its songs in Melue's version, available or not (greyed ones not counted). */
  entries: number
  /** Those that can be played. */
  available: number
  /** Linked: songs new in the file waiting to be added (greyed). */
  suggested: number
  /** Linked: songs removed in Melue that the file still has (greyed). */
  removedHere: number
  /** Linked: the file's name; null for app playlists. */
  file: string | null
  /** Linked: the file is gone (deleted, or its folder removed). Unlink to keep it as an app playlist. */
  fileMissing: boolean
  /** Linked: songs new in the file are 'added' (default) or 'suggested'. */
  newInFile: 'added' | 'suggested'
  /** Linked: songs removed from the file are 'removed' (default) or 'kept' (marked). */
  removedFromFile: 'removed' | 'kept'
  /** Linked: Melue's changes are written into the file (off by default). */
  writeBack: boolean
  /** Linked: following its file. Off: "Unfollowed" (the file's changes don't come in; greyed songs not shown). */
  following: boolean
  /** How long its available songs play. */
  durationMs: number
  /** Synced to the share (Melue's own playlists, once synced). */
  synced: boolean
  /** "Only on this device": not synced (setPlaylistLocalOnly in sync.ts). */
  localOnly: boolean
  /** Synced: songs another device added that aren't on this device yet (greyed, play once synced). */
  notHere: number
  /** The first (up to 4) different covers of its songs, for its picture (coverUrl). */
  covers: string[]
  /** Its own cover (set in Melue, for coverUrl), shown instead of its songs' covers. */
  customCover: string | null
  /** Linked: its file is in the synced folder (every device has it; its cover syncs too). */
  fileSynced: boolean
  /** A Spotify playlist (from its link): its id there, its songs there, and whether it changed there since its list was
   * last updated (one with more than 100 songs: update its list from a CSV). */
  /** A Spotify playlist: its id there, its songs there, changed there (the "!"), Spotify's picture (a cover file,
   * once downloaded), and whether its songs' covers show instead. */
  spotify: { id: string; total: number | null; changed: boolean; cover: string | null; songsCover: boolean } | null
  createdAt: number
  updatedAt: number
}

/**
 * How an entry of a linked playlist relates to its file. App playlists' entries are always 'both'.
 * - both: in Melue's version and in the file
 * - here_only: added in Melue, not in the file
 * - removed_here: removed in Melue, still in the file (greyed, doesn't play; restorePlaylistEntry)
 * - suggested: new in the file, not added yet (greyed, doesn't play; restorePlaylistEntry adds it)
 * - file_removed: removed from the file, kept in Melue (marked)
 */
export type PlaylistEntryState = 'both' | 'here_only' | 'removed_here' | 'suggested' | 'file_removed'

/** A linked playlist deleted with your changes, kept 30 days on this device to restore. */
export interface DeletedLinkedPlaylist {
  id: number
  name: string
  songs: number
  deletedAt: number
  /** The playlist file's name. */
  file: string
}

/** A playlist file let go of (its playlist unlinked or deleted). */
export interface LetGoFile {
  folderId: number
  /** Its music folder's name. */
  folder: string
  /** Where it is in that folder. */
  path: string
  /** The file isn't there any more: it can be forgotten, not followed. */
  gone: boolean
}

/** A song in a playlist. The same song can be in a playlist twice: each entry has its own id. */
export interface PlaylistEntry {
  id: number
  position: number
  addedAt: number
  state: PlaylistEntryState
  /** Linked: its line in the file. Synced, not on this device yet: its path on the share. */
  fileEntry: string | null
  /** Synced, not on this device yet: its title and artist (shown greyed). */
  syncTitle: string | null
  syncArtist: string | null
  /** Unavailable songs (missing files, folder switched off) stay, shown greyed. Null: a line of the file that matches no song. */
  track: Track | null
}

export interface PlaylistOptions {
  newInFile?: 'added' | 'suggested'
  removedFromFile?: 'removed' | 'kept'
  writeBack?: boolean
}

/** A song, with its tags as the file has them. */
export interface Track {
  id: number
  folderId: number
  /** The title tag, or the file's name. */
  title: string
  artist: string | null
  album: string | null
  albumArtist: string | null
  trackNo: number | null
  discNo: number | null
  year: number | null
  genre: string | null
  durationMs: number | null
  /** mp3, flac, m4a, ogg, opus, wav, ... */
  format: string | null
  bitrateKbps: number | null
  sampleRate: number | null
  /** Its cover, for coverUrl (null: none). */
  cover: string | null
  /** False until the scan's second pass has read its tags. */
  tagsRead: boolean
  /** Why its tags couldn't be read. */
  tagsError: string | null
  /** Whether it can be played (its file is there, its folder switched on). False only in playlists. */
  available: boolean
  liked: boolean
  /** How often it was played: to the end, or left after half of it or 4 minutes. */
  playCount: number
  /** Its album's page (null: not on an album). */
  albumId: number | null
  /** Where the file is in its folder (a content:// address for Android's media index and chosen folders). */
  path: string
  /** When the library first saw it (ms). */
  addedAt: number
  /** Its artists with their pages' ids: the main ones, then the featured ones ("A feat. B": A, then B). */
  artists: ArtistLink[]
}

/** How the Songs tab is sorted: title, artist and album A–Z; date added and plays, newest and most first. */
export type TrackSort = 'title' | 'artist' | 'album' | 'added' | 'plays'
/** How the Albums tab is sorted: title, artist, year (newest first), date added (newest first). */
export type AlbumSort = 'title' | 'artist' | 'year' | 'added'
/** How the Artists tab is sorted: name, or songs (most first). */
export type ArtistSort = 'name' | 'songs'

/** A song ready to play (the player's queue item): `id` is the song's library id. */
export interface QueueItem {
  id: number
  src: string
  title?: string
  artist?: string
  album?: string
  /** Its large cover, as a file path (for the notification and Android Auto). */
  artworkUrl?: string
}

/**
 * A list to play: 'songs' (all, by title; 'songs:plays' in the Songs tab's order), 'album:<id>', 'artist:<id>' (albums oldest first, then songs
 * not on an album), 'playlist:<id>', 'liked' (most recently liked first), 'folder:<id>' (a music folder, by
 * path), 'top:<period>' (the most played songs).
 */
export type Source =
  | 'songs'
  | `songs:${TrackSort}`
  | 'liked'
  | `album:${number}`
  | `artist:${number}`
  | `playlist:${number}`
  | `folder:${number}`
  | `top:${TopPeriod}`

/** The Top songs lists: the last 30 days, 6 months, year (rolling), and all time. */
export type TopPeriod = 'month' | 'half-year' | 'year' | 'all-time'

/** An auto playlist: All songs, Liked songs, the Top songs lists, and one per music folder. */
export interface AutoPlaylist {
  source: Source
  name: string
  kind: 'songs' | 'liked' | 'top' | 'folder'
  /** Songs in it now. */
  songs: number
  /** Top songs: how many it holds (25, 50, 100 or 200). */
  size: number | null
  /** Hidden: shown only in the settings, to show it again. */
  hidden: boolean
}

/** The sizes a Top songs list can have. */
export const AUTO_PLAYLIST_SIZES = [25, 50, 100, 200] as const

/** What leaving a song counted as: a play (to the end, or after half or 4 minutes), a skip (in its first 30 s), or neither. */
export type PlaybackOutcome = 'play' | 'skip' | 'neither'

/** How the queue follows library changes to the list it plays (the player's updateQueue options). */
export interface QueueFollow {
  enabled: boolean
  /** The playing song left the list: 'finish' it (default), or 'skip' to the next now. */
  removedCurrent: 'finish' | 'skip'
  /** Other songs that left it: 'remove' (default) or 'keep'. */
  removedItems: 'remove' | 'keep'
  /** New songs: 'inPlace' (default), 'end' or 'next'. */
  newItems: 'inPlace' | 'end' | 'next'
  /** The queue's order: the 'playlist's (the list's, default) or the 'queue's own. */
  order: 'playlist' | 'queue'
}

/** The player's settings, as Melue keeps them (library/player_settings.rs); settings/playerSettings.ts hands them to the player. */
export interface PlayerSettings {
  volumeCurve: 'linear' | 'quadratic' | 'cubic'
  /** Desktop: the system's media controls and the media keys. */
  mediaControls: boolean
  /** What previous / next do on media keys, headphones, the notification: 0 skips songs, above 0 seeks that many seconds. */
  skipSeconds: number
  /** The last queue when Melue starts: loaded and 'paused', 'play', or 'off' (starts empty). */
  resumeLastQueue: 'off' | 'paused' | 'play'
  /** 0: previous always goes to the song before. */
  previousRestartsAfterSeconds: number
  /** Next on the last song, with repeat off. */
  nextAtEnd: 'nothing' | 'first'
  repeatAddedTracks: boolean
  /** Android: how long the player stays ready after a pause. */
  pausedKeepAliveMinutes: number
  /** Android: stays ready while Android Auto is connected. */
  keepAliveWhileCarConnected: boolean
  /** Android: offload (play on the audio chip, saving battery) also formats the phone can only offload with gaps. */
  offloadWithGaps: boolean
}

/** An album: songs with the same album title and album artist (see the grouping rules in grouping.rs). */
export interface Album {
  id: number
  title: string
  /** The album artist; null for a compilation (various artists). */
  artist: string | null
  /** The album artist's page. */
  artistId: number | null
  compilation: boolean
  /** The year most of its songs have. */
  year: number | null
  /** The cover most of its songs have, for coverUrl. */
  cover: string | null
  /** Its songs that are there. */
  tracks: number
}

/** An artist a song names, and their page's id. */
export interface ArtistLink {
  id: number
  name: string
}

/** An artist, from the songs' artist tags ("A feat. B" is A, with B featured). */
export interface Artist {
  id: number
  name: string
  /** Songs they're a main artist of. */
  tracks: number
  /** Songs they're featured on. */
  featured: number
  /** Albums they're the album artist of. */
  albums: number
}

/** An album's page: the album, and its songs in the order they play (by disc, then track number). */
export interface AlbumPage {
  album: Album
  tracks: Track[]
}

/**
 * An artist's page: the artist, their albums (oldest first), their songs (as their albums go, then the ones not on an
 * album) and the songs they're featured on.
 */
export interface ArtistPage {
  artist: Artist
  albums: Album[]
  tracks: Track[]
  featured: Track[]
}

export interface FolderCovers {
  /** Whether all music on the phone uses cover images next to the songs (cover.jpg, folder.jpg). */
  enabled: boolean
  /** The photos permission they need. Always 'granted' on desktop. */
  permission: 'granted' | 'denied' | 'prompt' | 'prompt-with-rationale'
}

/** The address of a cover: small (300 px, lists and grids) or large (1000 px, the player). */
export const coverUrl = (cover: string, size: 'small' | 'large' = 'small') =>
  convertFileSrc(`${cover}-${size === 'small' ? 300 : 1000}.jpg`, 'cover')

/**
 * A song whose file can't be found (deleted, moved, or on a drive that isn't there now): kept in the library, with its
 * likes, plays and playlist places, in case it comes back.
 */
export interface MissingSong {
  id: number
  title: string
  artist: string | null
  /** Its music folder's name, and where the file was in it. */
  folder: string
  path: string
  missingSince: number
}

export const library = {
  status: () => invoke<LibraryStatus>('library_status'),
  /**
   * Music folders that are the share, inside it, or look like it (a network folder of nearly all synced songs' copies):
   * Melue never removes files from them. Their ids and why.
   */
  shareFolders: () => invoke<[number, 'share' | 'insideShare' | 'looksLikeShare'][]>('library_share_folders'),
  /** The songs whose files can't be found (kept, in case they come back). */
  missingSongs: () => invoke<MissingSong[]>('library_missing_songs'),
  /** An app setting saved with the library, or null when it was never set. */
  setting: (key: string) => invoke<string | null>('library_setting', { key }),
  setSetting: (key: string, value: string) => invoke<void>('library_set_setting', { key, value }),

  folders: () => invoke<MusicFolder[]>('library_folders'),
  /**
   * Adds a music folder. Refused when it's already there, inside or around one that is, or (Android) when
   * all music on the phone and chosen folders would both be on. 'android_media' asks for the permission.
   */
  addFolder: (kind: FolderKind, location: string, name?: string) =>
    invoke<MusicFolder>('library_add_folder', { kind, location, name }),
  /** Removes a folder and its songs from the library (the files stay). To keep them, switch it off. */
  removeFolder: (id: number) => invoke<void>('library_remove_folder', { id }),
  setFolderEnabled: (id: number, enabled: boolean) => invoke<MusicFolder>('library_set_folder_enabled', { id, enabled }),
  /** An empty name goes back to the folder's own name. */
  renameFolder: (id: number, name: string) => invoke<MusicFolder>('library_rename_folder', { id, name }),
  /**
   * Scans every switched-on folder (a refresh button) and resolves with what changed, or null when a scan
   * was already running (it goes once more). Melue also scans at start-up and, on Android, after changes.
   */
  scan: () => invoke<ScanSummary | null>('library_scan'),
  /** Calls `listener` with every scan's progress. Resolves to a function that stops listening. */
  onScan: (listener: (event: ScanEvent) => void) => listen<ScanEvent>('library-scan', (e) => listener(e.payload)),
  /**
   * Calls `listener` whenever the library changes, with what changed (refresh what shows those songs or
   * lists). Resolves to a function that stops listening.
   */
  onChange: (listener: (change: LibraryChange) => void) =>
    listen<LibraryChange>('library-changed', (e) => listener(e.payload)),
  /** Android: whether Melue may read the music on the phone, asking first with `request`. */
  musicAccess: (request = false) => invoke<MusicAccess>('library_music_access', { request }),

  playlists: () => invoke<Playlist[]>('library_playlists'),
  playlist: (id: number) => invoke<Playlist>('library_playlist', { id }),
  playlistEntries: (id: number) => invoke<PlaylistEntry[]>('library_playlist_entries', { id }),
  /** A Spotify playlist's songs' links on Spotify, by their entries' `fileEntry` (the ones known). */
  spotifyLinks: (id: number) => invoke<Record<string, string>>('library_spotify_links', { id }),
  createPlaylist: (name: string) => invoke<Playlist>('library_create_playlist', { name }),
  renamePlaylist: (id: number, name: string) => invoke<Playlist>('library_rename_playlist', { id, name }),
  /** Deletes a playlist (its songs stay in the library). */
  deletePlaylist: (id: number) => invoke<void>('library_delete_playlist', { id }),
  /** Adds songs at `position` (from 0; the end when left out). Resolves with the new entries' ids. */
  addToPlaylist: (id: number, tracks: number[], position?: number) =>
    invoke<number[]>('library_add_to_playlist', { id, tracks, position }),
  /** Removes entries (by entry id: one copy of a song that's in twice). */
  removeFromPlaylist: (id: number, entries: number[]) => invoke<void>('library_remove_from_playlist', { id, entries }),
  /** Find song: `track` for the playlist's entry `entry` (one not here); kept, and synced to the other devices. */
  choosePlaylistSong: (id: number, entry: number, track: number) => invoke<void>('library_choose_playlist_song', { id, entry, track }),
  moveInPlaylist: (id: number, entry: number, position: number) =>
    invoke<void>('library_move_in_playlist', { id, entry, position }),
  /** Linked: takes a greyed song (removed here, or suggested by the file) into Melue's version. */
  restorePlaylistEntry: (id: number, entry: number) => invoke<void>('library_restore_playlist_entry', { id, entry }),
  /** Linked: its settings (left out: unchanged). Switching write-back on writes the file right away. */
  setPlaylistOptions: (id: number, options: PlaylistOptions) =>
    invoke<Playlist>('library_set_playlist_options', { id, ...options }),
  /** Linked: keeps it as an app playlist; the file is left alone and not linked again. */
  unlinkPlaylist: (id: number) => invoke<Playlist>('library_unlink_playlist', { id }),
  /** Playlist files let go of (unlinked, or their playlist deleted): not linked again until followed again. */
  letGoFiles: () => invoke<LetGoFile[]>('library_let_go_files'),
  /** Gives a playlist its own cover: a picture (JPEG, PNG or WebP), cropped square. */
  setPlaylistCover: (id: number, picture: Uint8Array) => invoke<Playlist>('library_set_playlist_cover', { id, picture: Array.from(picture) }),
  /** Back to a playlist's default cover (its songs' covers). */
  clearPlaylistCover: (id: number) => invoke<Playlist>('library_clear_playlist_cover', { id }),
  /**
   * Copies music files into a music folder on this computer (`Artist/Album/`; the files stay where they are), then
   * scans them in; `playlist`: they go into it too, once found. `failed`: (file name, why).
   */
  addSongsToFolder: (folder: number, files: string[], playlist: number | null) =>
    invoke<{ added: number; failed: [string, string][] }>('library_add_songs_to_folder', { folder, files, playlist }),
  /** A picture file's bytes (JPEG, PNG or WebP, up to 30 MB): one dropped on Melue. */
  readPicture: (path: string) => invoke<number[]>('library_read_picture', { path }).then((bytes) => new Uint8Array(bytes)),
  /** Takes a let-go file whose file is gone off the list. */
  forgetLetGoFile: (folderId: number, path: string) => invoke<void>('library_forget_let_go_file', { folderId, path }),
  /** Search: songs (the first `limit`, and how many in all), albums, artists and playlists whose names have `query`. */
  search: (query: string, limit = 200) =>
    invoke<{ tracks: Track[]; trackCount: number; albums: Album[]; artists: Artist[]; playlists: Playlist[] }>('library_search', { query, limit }),
  /** The playlists a song is in (Add to playlist's ✓). */
  playlistsWithTrack: (id: number) => invoke<number[]>('library_playlists_with_track', { id }),
  /** Exports a list's songs as an .m3u8 file at `path`. Resolves with how many songs it holds. */
  exportPlaylist: (source: Source, path: string) => invoke<number>('library_export_playlist', { source, path }),
  /** Stops or starts following a linked playlist's file (following again shows how it differs from the file). */
  setPlaylistFollowing: (id: number, following: boolean) => invoke<void>('library_set_playlist_following', { id, following }),
  /** Linked playlists deleted with your changes in the last 30 days (this device). */
  deletedLinkedPlaylists: () => invoke<DeletedLinkedPlaylist[]>('library_deleted_linked_playlists'),
  /** Restores one as it was (replacing the file's playlist if it was followed again). Resolves with its id. */
  restoreLinkedPlaylist: (id: number) => invoke<number>('library_restore_linked_playlist', { id }),
  /** Follows a let-go playlist file again: it's linked as a new playlist (scans now). */
  followFileAgain: (folderId: number, path: string) => invoke<void>('library_follow_file_again', { folderId, path }),

  /** Songs that are there, by title. */
  /** Songs that are there, sorted (by title unless asked otherwise). */
  tracks: (offset = 0, limit = 100, sort: TrackSort = 'title') => invoke<Track[]>('library_tracks', { offset, limit, sort }),
  /** Songs by when they were last played, latest first. */
  recentlyPlayed: (limit = 50) => invoke<Track[]>('library_recently_played', { limit }),
  /** Songs by play count, most first. */
  mostPlayed: (limit = 50) => invoke<Track[]>('library_most_played', { limit }),
  /** Every auto playlist, in order, hidden ones too (marked). */
  autoPlaylists: () => invoke<AutoPlaylist[]>('library_auto_playlists'),
  /** Hides an auto playlist or shows it again, and (Top songs) changes its size. Resolves with them all. */
  setAutoPlaylist: (source: Source, change: { hidden?: boolean; size?: number }) =>
    invoke<AutoPlaylist[]>('library_set_auto_playlist', { source, ...change }),

  /** A list's songs that can be played, in its order, ready for the player (see playSource). */
  queueItems: (source: Source) => invoke<QueueItem[]>('library_queue_items', { source }),
  /** Songs ready to play, in the order given (ones not there left out): Play next, Add to queue. */
  queueItemsFor: (ids: number[]) => invoke<QueueItem[]>('library_queue_items_for', { ids }),
  /** Use setLiked from '@/library/playback' instead: it also updates the player's like button. */
  setLikedOnly: (id: number, liked: boolean) => invoke<void>('library_set_liked', { id, liked }),
  /** Forgets songs that can't be found (these, or all): out of the library, with their likes and plays. How many. */
  forgetMissing: (ids?: number[]) => invoke<number>('library_forget_missing', { ids: ids ?? null }),
  /** Spotify playlists (src-tauri library/spotify.rs): one from its link (a public playlist), its id here. */
  addSpotifyPlaylist: (link: string) => invoke<number>('library_add_spotify_playlist', { link }),
  /** A Spotify playlist shows its songs' covers (`songs`) or Spotify's picture, on every device. */
  setSongsCover: (id: number, songs: boolean) => invoke<void>('library_set_songs_cover', { id, songs }),
  /** Read again now (one, or all): whether any changed here. */
  updateSpotifyPlaylists: (id?: number) => invoke<boolean>('library_update_spotify_playlists', { id: id ?? null }),
  /** A long one's full list from a CSV: its text (a phone's picked file), or a .csv file's path (a computer). */
  importSpotifyCsv: (id: number, csv: string) => invoke<void>('library_import_spotify_csv', { id, csv }),
  importSpotifyCsvFile: (id: number, path: string) => invoke<void>('library_import_spotify_csv_file', { id, path }),
  /** Computers: waits for the CSV just exported to land in Downloads, and imports it (true), or none came (false). */
  waitForSpotifyCsv: (id: number) => invoke<boolean>('library_wait_for_spotify_csv', { id }),
  stopWaitingForSpotifyCsv: () => invoke<void>('library_stop_waiting_for_spotify_csv'),
  likedIds: () => invoke<number[]>('library_liked_ids'),
  queueFollow: () => invoke<QueueFollow>('library_queue_follow'),
  setQueueFollow: (settings: QueueFollow) => invoke<QueueFollow>('library_set_queue_follow', { settings }),
  playerSettings: () => invoke<PlayerSettings>('library_player_settings'),
  /** The songs with these ids, in no particular order (the player's queue: lengths, likes, covers). */
  tracksByIds: (ids: number[]) => invoke<Track[]>('library_tracks_by_ids', { ids }),
  /** How much space the covers take, in bytes. */
  coversSize: () => invoke<number>('library_covers_size'),
  /** Removes covers no song uses any more; resolves with how many files went. */
  cleanCovers: () => invoke<number>('library_clean_covers'),
  setPlayerSettings: (settings: PlayerSettings) => invoke<PlayerSettings>('library_set_player_settings', { settings }),
  /** Albums with songs that are there, sorted (by title, "The" ignored, unless asked otherwise). */
  albums: (offset = 0, limit = 100, sort: AlbumSort = 'title') => invoke<Album[]>('library_albums', { offset, limit, sort }),
  /** Artists with songs that are there, sorted (by name, "The" and accents ignored or their sort tag, unless asked). */
  artists: (offset = 0, limit = 100, sort: ArtistSort = 'name') => invoke<Artist[]>('library_artists', { offset, limit, sort }),
  /** An album's page (null: no such album). */
  albumPage: (id: number) => invoke<AlbumPage | null>('library_album_page', { id }),
  /** An artist's page (null: no such artist). */
  artistPage: (id: number) => invoke<ArtistPage | null>('library_artist_page', { id }),
  /** A Made for you list's songs, in its order: 'songs' (A–Z), 'liked' (newest liked first), 'top:<period>'. */
  autoPlaylistTracks: (source: Source) => invoke<Track[]>('library_auto_playlist_tracks', { source }),
  /** A music folder's songs, by where they are in it. */
  folderTracks: (id: number) => invoke<Track[]>('library_folder_tracks', { id }),
  /** Desktop: opens the song's folder in the file manager with the file selected. */
  showInFolder: (id: number) => invoke<void>('library_show_in_folder', { id }),
  /** Android, all music on the phone: whether cover images next to the songs are used (off by default). */
  folderCovers: () => invoke<FolderCovers>('library_folder_covers'),
  /** Switching on asks for the photos permission, and is refused without it. Rescans either way. */
  setFolderCovers: (enabled: boolean) => invoke<FolderCovers>('library_set_folder_covers', { enabled }),
}
