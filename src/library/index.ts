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

export const library = {
  status: () => invoke<LibraryStatus>('library_status'),
  /** An app setting saved with the library, or null when it was never set. */
  setting: (key: string) => invoke<string | null>('library_setting', { key }),
  setSetting: (key: string, value: string) => invoke<void>('library_set_setting', { key, value }),
}
