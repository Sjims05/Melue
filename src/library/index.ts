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
  reason: 'scan' | 'folders' | 'playlists'
  /** Songs added, changed (tags, cover, file), gone (missing, removed, hidden with their folder) or back. */
  tracks: number[]
  /** The playable lists whose songs changed: 'songs' (all songs), 'album:<id>', 'artist:<id>', 'playlist:<id>'. */
  sources: string[]
}

/** A playlist made in Melue ('app'), or a playlist file linked into the library ('linked', coming next). */
export interface Playlist {
  id: number
  name: string
  kind: 'app' | 'linked'
  /** Its entries, available or not. */
  entries: number
  /** Entries that can be played. */
  available: number
  /** How long its available songs play. */
  durationMs: number
  /** The first (up to 4) different covers of its songs, for its picture (coverUrl). */
  covers: string[]
  createdAt: number
  updatedAt: number
}

/** A song in a playlist. The same song can be in a playlist twice: each entry has its own id. */
export interface PlaylistEntry {
  id: number
  position: number
  addedAt: number
  /** Unavailable songs (missing files, folder switched off) stay, shown greyed. */
  track: Track
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
}

/** An album: songs with the same album title and album artist (see the grouping rules in grouping.rs). */
export interface Album {
  id: number
  title: string
  /** The album artist; null for a compilation (various artists). */
  artist: string | null
  compilation: boolean
  /** The year most of its songs have. */
  year: number | null
  /** The cover most of its songs have, for coverUrl. */
  cover: string | null
  /** Its songs that are there. */
  tracks: number
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

export interface FolderCovers {
  /** Whether all music on the phone uses cover images next to the songs (cover.jpg, folder.jpg). */
  enabled: boolean
  /** The photos permission they need. Always 'granted' on desktop. */
  permission: 'granted' | 'denied' | 'prompt' | 'prompt-with-rationale'
}

/** The address of a cover: small (300 px, lists and grids) or large (1000 px, the player). */
export const coverUrl = (cover: string, size: 'small' | 'large' = 'small') =>
  convertFileSrc(`${cover}-${size === 'small' ? 300 : 1000}.jpg`, 'cover')

export const library = {
  status: () => invoke<LibraryStatus>('library_status'),
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
  createPlaylist: (name: string) => invoke<Playlist>('library_create_playlist', { name }),
  renamePlaylist: (id: number, name: string) => invoke<Playlist>('library_rename_playlist', { id, name }),
  /** Deletes a playlist (its songs stay in the library). */
  deletePlaylist: (id: number) => invoke<void>('library_delete_playlist', { id }),
  /** Adds songs at `position` (from 0; the end when left out). Resolves with the new entries' ids. */
  addToPlaylist: (id: number, tracks: number[], position?: number) =>
    invoke<number[]>('library_add_to_playlist', { id, tracks, position }),
  /** Removes entries (by entry id: one copy of a song that's in twice). */
  removeFromPlaylist: (id: number, entries: number[]) => invoke<void>('library_remove_from_playlist', { id, entries }),
  moveInPlaylist: (id: number, entry: number, position: number) =>
    invoke<void>('library_move_in_playlist', { id, entry, position }),

  /** Songs that are there, by title. */
  tracks: (offset = 0, limit = 100) => invoke<Track[]>('library_tracks', { offset, limit }),
  /** Albums with songs that are there, by title ("The" ignored). */
  albums: (offset = 0, limit = 100) => invoke<Album[]>('library_albums', { offset, limit }),
  /** Artists with songs that are there, by name ("The" and accents ignored, or their sort tag). */
  artists: (offset = 0, limit = 100) => invoke<Artist[]>('library_artists', { offset, limit }),
  /** Android, all music on the phone: whether cover images next to the songs are used (off by default). */
  folderCovers: () => invoke<FolderCovers>('library_folder_covers'),
  /** Switching on asks for the photos permission, and is refused without it. Rescans either way. */
  setFolderCovers: (enabled: boolean) => invoke<FolderCovers>('library_set_folder_covers', { enabled }),
}
