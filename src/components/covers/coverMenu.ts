/**
 * A cover card's right-click menu (long-press on a phone): what it offers depends on what the card is. Everything
 * plays, shuffles, plays next and goes in the queue; then an album goes to its artist; your playlist renames,
 * changes where it lives (only here / synced / linked) and deletes; an auto playlist hides. A song's card uses the
 * song's own menu (songMenu): Add to playlist is for songs only.
 */
import { Cloud, EyeOff, ListEnd, Lock, Pencil, Shuffle, Trash2, Unlink, User } from '@lucide/vue'
import { NextIcon, PlayIcon } from '@/components/player/icons'
import type { Component } from 'vue'

export type CoverAction =
  | 'play'
  | 'shuffle'
  | 'playNext'
  | 'addToQueue'
  | 'goToArtist'
  | 'rename'
  | 'keepHereOnly'
  | 'syncEverywhere'
  | 'unlink'
  | 'delete'
  | 'hide'

/** What the card is; a playlist also says where it lives. */
export type CoverKind =
  | { kind: 'album' }
  | { kind: 'playlist'; lives: 'synced' | 'here' | 'linked' }
  | { kind: 'auto' }
  /** An artist or a music folder (their rows): only playing them. */
  | { kind: 'list' }

/** One item in a card's (or song's) menu. */
export interface MenuItem<A extends string = string> {
  action: A
  label: string
  icon: Component
  /** The icon filled (Remove from liked). */
  filled?: boolean
  /** It takes something away (red). */
  danger?: boolean
  /** A line above it. */
  apart?: boolean
}

export function coverMenu(what: CoverKind): MenuItem<CoverAction>[] {
  const items: MenuItem<CoverAction>[] = [
    { action: 'play', label: 'Play', icon: PlayIcon },
    { action: 'shuffle', label: 'Shuffle', icon: Shuffle },
    { action: 'playNext', label: 'Play next', icon: NextIcon },
    { action: 'addToQueue', label: 'Add to queue', icon: ListEnd },
  ]
  if (what.kind === 'album') {
    items.push({ action: 'goToArtist', label: 'Go to artist', icon: User, apart: true })
  } else if (what.kind === 'playlist') {
    items.push(
      { action: 'rename', label: 'Rename', icon: Pencil, apart: true },
      what.lives === 'synced'
        ? { action: 'keepHereOnly', label: 'Keep only on this device', icon: Lock }
        : what.lives === 'here'
          ? { action: 'syncEverywhere', label: 'Sync to your other devices', icon: Cloud }
          : { action: 'unlink', label: 'Unlink', icon: Unlink },
      { action: 'delete', label: 'Delete…', icon: Trash2, danger: true, apart: true },
    )
  } else if (what.kind === 'auto') {
    items.push({ action: 'hide', label: 'Hide', icon: EyeOff, apart: true })
  }
  return items
}
