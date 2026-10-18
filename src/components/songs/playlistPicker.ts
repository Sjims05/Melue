/**
 * Add to playlist, for the song menus: the app hands the menus what it needs (provide PLAYLIST_PICKER): your
 * playlists (✓ on the ones that have the song), adding to one, and a new one with the song in it. Without it (the
 * component library), the menus leave Add to playlist out.
 */
import type { InjectionKey } from 'vue'
import type { Song } from './songMenu'

export interface PlaylistChoice {
  id: number
  name: string
  /** The song is in it already (✓; picking it adds it again). */
  has: boolean
}

export interface PlaylistPicker {
  list: (song: Song) => Promise<PlaylistChoice[]>
  add: (song: Song, playlist: number) => Promise<void>
  /** A new playlist with the song in it (its name then ready to type). */
  create: (song: Song) => Promise<void>
}

export const PLAYLIST_PICKER: InjectionKey<PlaylistPicker> = Symbol('playlist picker')
