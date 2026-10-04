/**
 * Melue's music library, kept by the app's Rust side (src-tauri/src/library): the database, scanning
 * and tags. The UI uses these instead of calling the commands itself.
 */
import { invoke } from '@tauri-apps/api/core'

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
  /** Android: whether Melue may read the music on the phone, asking first with `request`. */
  musicAccess: (request = false) => invoke<MusicAccess>('library_music_access', { request }),
}
