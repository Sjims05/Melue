/**
 * A song's menu (its ⋯ button and right-click): what it offers depends on the song and where it is (in a playlist,
 * on a desktop, with a share). The same items for both menus.
 */
import { CloudUpload, Disc3, FolderOpen, Heart, ListEnd, ListMusic, ListX, SkipForward, User } from '@lucide/vue'
import type { MenuItem } from '@/components/covers/coverMenu'

export interface Song {
  id: number | string
  title: string
  artist: string
  album?: string
  /** Seconds. */
  duration: number
  /** The cover's address (Melue's cover:// or any image); none: a plain tile. */
  cover?: string
  liked?: boolean
}

export type SongAction =
  | 'playNext'
  | 'addToQueue'
  | 'like'
  | 'goToAlbum'
  | 'goToArtist'
  | 'addToPlaylist'
  | 'showInFolder'
  | 'moveToSync'
  | 'removeFromPlaylist'

/** Where the song is, for the menu's extras. */
export interface SongMenuPlace {
  /** In a playlist: Remove from playlist. */
  inPlaylist?: boolean
  /** Desktop: Show in folder. */
  showInFolder?: boolean
  /** A local song, with a share: Move to sync. */
  moveToSync?: boolean
}

/** One item in a song's menu (the same shape as a cover card's). */
export type SongMenuItem = MenuItem<SongAction>

export function songMenu(song: Song, place: SongMenuPlace = {}): SongMenuItem[] {
  const items: SongMenuItem[] = [
    { action: 'playNext', label: 'Play next', icon: SkipForward, filled: true, small: true },
    { action: 'addToQueue', label: 'Add to queue', icon: ListEnd },
    { action: 'like', label: song.liked ? 'Remove from liked' : 'Add to liked', icon: Heart, filled: song.liked },
    ...(song.album ? [{ action: 'goToAlbum' as const, label: 'Go to album', icon: Disc3 }] : []),
    { action: 'goToArtist', label: 'Go to artist', icon: User },
    { action: 'addToPlaylist', label: 'Add to playlist', icon: ListMusic },
  ]
  const extras: SongMenuItem[] = [
    ...(place.showInFolder ? [{ action: 'showInFolder' as const, label: 'Show in folder', icon: FolderOpen }] : []),
    ...(place.moveToSync ? [{ action: 'moveToSync' as const, label: 'Move to sync', icon: CloudUpload }] : []),
    ...(place.inPlaylist ? [{ action: 'removeFromPlaylist' as const, label: 'Remove from playlist', icon: ListX, danger: true }] : []),
  ]
  if (extras.length) extras[0].apart = true
  return [...items, ...extras]
}
