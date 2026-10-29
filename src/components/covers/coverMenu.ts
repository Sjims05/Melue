/**
 * A cover card's right-click menu (long-press on a phone): what it offers depends on what the card is. Everything
 * plays, shuffles, plays next and goes in the queue; then an album goes to its artist; your playlist renames,
 * changes where it lives (only here / synced; a linked one stops following its file), deletes, and explains how it
 * lives (About syncing / "This device" / following a file / Spotify; a Spotify one also copies the songs you don't have);
 * an auto playlist hides. A song's card uses the
 * song's own menu (songMenu): Add to playlist is for songs only.
 */
import { AudioLines, ClipboardList, Cloud, EyeOff, FileDown, ImageMinus, ImagePlus, Info, LayoutGrid, Link, Link2, ListEnd, Lock, Pencil, Shuffle, Trash2, Unlink, User } from '@lucide/vue'
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
  | 'copyMissing'
  | 'copyMissingLinks'
  | 'follow'
  | 'delete'
  | 'hide'
  | 'about'
  | 'export'
  | 'changeCover'
  | 'revertCover'
  | 'songsCover'
  | 'spotifyCover'

/** What the card is; a playlist also says where it lives. */
export type CoverKind =
  | { kind: 'album' }
  /**
   * A playlist; `customCover`: it has its own cover (Revert to default cover). A Spotify one also says where it
   * lives (`synced`: with a share; true synced, false only here) and, without its own cover, which picture it shows
   * (`spotifyCover`: Spotify's or its songs'; the other one is offered).
   */
  | {
      kind: 'playlist'
      lives: 'synced' | 'here' | 'linked' | 'unfollowed' | 'spotify' | 'plain'
      customCover?: boolean
      synced?: boolean
      spotifyCover?: 'spotify' | 'songs'
    }
  | { kind: 'auto' }
  /** An artist or a music folder (their rows): only playing them. */
  | { kind: 'list' }

/** One item in a card's (or song's) menu. */
export interface MenuItem<A extends string = string> {
  action: A
  label: string
  /** None: a choice in a phone's choice drawer (DropValue). */
  icon?: Component
  /** The icon filled (Remove from liked). */
  filled?: boolean
  /** It takes something away (red). */
  danger?: boolean
  /** A line above it. */
  apart?: boolean
}

/** `exportable`: a playlist's or a Made for you list's Export as .m3u8 (desktop). */
export function coverMenu(what: CoverKind, exportable = false): MenuItem<CoverAction>[] {
  const items: MenuItem<CoverAction>[] = [
    { action: 'play', label: 'Play', icon: PlayIcon },
    { action: 'shuffle', label: 'Shuffle', icon: Shuffle },
    { action: 'playNext', label: 'Play next', icon: NextIcon },
    { action: 'addToQueue', label: 'Add to queue', icon: ListEnd },
  ]
  if (what.kind === 'album') {
    items.push({ action: 'goToArtist', label: 'Go to artist', icon: User, apart: true })
  } else if (what.kind === 'playlist') {
    // Where it lives: synced (or not, with a share), linked to a file; 'plain': no share, nothing to choose.
    const lives: MenuItem<CoverAction>[] =
      what.lives === 'synced'
        ? [{ action: 'keepHereOnly', label: 'Keep only on this device…', icon: Lock }]
        : what.lives === 'here'
          ? [{ action: 'syncEverywhere', label: 'Sync to your other devices', icon: Cloud }]
          : what.lives === 'linked'
            ? [{ action: 'unlink', label: 'Stop following the file…', icon: Unlink }]
            : what.lives === 'unfollowed'
              ? [{ action: 'follow', label: 'Follow the file again', icon: Link }]
              : what.lives === 'spotify'
                ? [
                    ...(what.synced === true
                      ? [{ action: 'keepHereOnly' as const, label: 'Keep only on this device…', icon: Lock }]
                      : what.synced === false
                        ? [{ action: 'syncEverywhere' as const, label: 'Sync to your other devices', icon: Cloud }]
                        : []),
                    { action: 'copyMissing', label: 'Copy the songs you don’t have', icon: ClipboardList },
                    { action: 'copyMissingLinks', label: 'Copy their Spotify links', icon: Link2 },
                    { action: 'unlink', label: 'Stop following Spotify…', icon: Unlink },
                  ]
                : []
    const about = { synced: 'About syncing', here: 'About “This device”', linked: 'About following a file', unfollowed: 'About “Unfollowed”', spotify: 'About Spotify playlists', plain: '' }[
      what.lives
    ]
    items.push(
      { action: 'rename', label: 'Rename', icon: Pencil, apart: true },
      { action: 'changeCover', label: 'Change cover…', icon: ImagePlus },
      ...(what.customCover ? [{ action: 'revertCover' as const, label: 'Revert to default cover', icon: ImageMinus }] : []),
      ...(what.spotifyCover === 'spotify' ? [{ action: 'songsCover' as const, label: 'Use the songs’ covers', icon: LayoutGrid }] : []),
      ...(what.spotifyCover === 'songs' ? [{ action: 'spotifyCover' as const, label: 'Use Spotify’s cover', icon: AudioLines }] : []),
      ...lives,
      { action: 'delete', label: 'Delete…', icon: Trash2, danger: true, apart: true },
      ...(about ? [{ action: 'about' as const, label: about, icon: Info, apart: true }] : []),
    )
  } else if (what.kind === 'auto') {
    items.push({ action: 'hide', label: 'Hide', icon: EyeOff, apart: true })
  }
  if (exportable && (what.kind === 'playlist' || what.kind === 'auto')) {
    // After Rename (a playlist), before Delete.
    const at = items.findIndex((i) => i.action === 'delete' || i.action === 'hide')
    items.splice(at, 0, { action: 'export', label: 'Export as .m3u8…', icon: FileDown, apart: what.kind === 'playlist' })
    if (what.kind === 'auto') items[at + 1] = { ...items[at + 1], apart: false }
  }
  return items
}
