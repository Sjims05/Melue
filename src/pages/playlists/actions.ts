/**
 * What your playlists' menus do, the same on the Playlists page and a playlist's own page: play it (and the rest of
 * the playing items), rename it (its name turns into a text field: `renaming`), keep it only here (asked first: the
 * other devices keep theirs, or it's taken off them) or sync it, stop following its file or Spotify (asked first)
 * or follow it again, delete it (asked first), update a Spotify playlist from Spotify (a long one changed there: the
 * CSV's steps, `spotifyUpdate`), copy a Spotify playlist's songs you don't have ("Artist – Title"
 * lines, or their links on Spotify: pasted into a playlist in Spotify on a computer, they add them), and the About items (`asking`, shown by PlaylistDialogs).
 */
import { ClipboardList, Link2, RefreshCw } from '@lucide/vue'
import { ref } from 'vue'
import type { CoverAction } from '@/components/covers'
import { library, type Playlist } from '@/library'
import { librarySync } from '@/library/sync'
import { toast } from '@/lib/toast'
import { exportList, listAction } from '@/player/songs'
import { readSpotify } from './playlists'

/** A question or explanation about a playlist: delete it? keep it only here? stop following its file? about how it
 * lives. */
export type PlaylistAsk = { what: 'delete' | 'keepHereOnly' | 'unlink' | 'about'; playlist: Playlist }

export function usePlaylistActions(changed: () => unknown, deleted?: () => unknown) {
  const renaming = ref<number | null>(null)
  const asking = ref<PlaylistAsk | null>(null)
  // Change cover: the playlist whose cover is being chosen (the crop window).
  const covering = ref<Playlist | null>(null)
  // A long Spotify playlist that changed there: its update's steps (a CSV) shown.
  const spotifyUpdate = ref<Playlist | null>(null)

  async function act(p: Playlist, action: CoverAction) {
    if (action === 'rename') renaming.value = p.id
    else if (action === 'delete' || action === 'keepHereOnly' || action === 'unlink' || action === 'about') asking.value = { what: action, playlist: p }
    else if (action === 'syncEverywhere') {
      await librarySync.setPlaylistLocalOnly(p.id, false)
      await changed()
    } else if (action === 'changeCover') {
      covering.value = p
    } else if (action === 'songsCover' || action === 'spotifyCover') {
      await library.setSongsCover(p.id, action === 'songsCover')
      await changed()
    } else if (action === 'revertCover') {
      await library.clearPlaylistCover(p.id)
      await changed()
    } else if (action === 'export') {
      await exportList(`playlist:${p.id}`, p.name)
    } else if (action === 'updateSpotify') {
      await updateFromSpotify(p)
    } else if (action === 'copyMissing') {
      await copyMissing(p)
    } else if (action === 'copyMissingLinks') {
      await copyMissingLinks(p)
    } else if (action === 'follow') {
      await library.setPlaylistFollowing(p.id, true)
      await changed()
    } else await listAction(`playlist:${p.id}`, action)
  }
  async function rename(p: Playlist, name: string) {
    renaming.value = null
    if (name !== p.name) await library.renamePlaylist(p.id, name)
    await changed()
  }
  /** `removeElsewhere`: Keep only on this device's second choice (deleted from the other devices). */
  async function confirm(ask: PlaylistAsk, removeElsewhere = false) {
    if (ask.what === 'keepHereOnly') {
      await librarySync.setPlaylistLocalOnly(ask.playlist.id, true, removeElsewhere)
      await changed()
    } else if (ask.what === 'delete') {
      await library.deletePlaylist(ask.playlist.id)
      await (deleted ?? changed)()
    } else if (ask.what === 'unlink') {
      // A Spotify playlist becomes your own (it isn't read from Spotify any more); a file's is kept, unfollowed.
      if (ask.playlist.spotify) await library.unlinkPlaylist(ask.playlist.id)
      else await library.setPlaylistFollowing(ask.playlist.id, false)
      await changed()
    }
  }
  async function copyMissing(p: Playlist) {
    const missing = (await library.playlistEntries(p.id)).filter((e) => !e.track && e.syncTitle)
    if (!missing.length) return toast('You have all its songs', ClipboardList)
    const lines = missing.map((e) => (e.syncArtist ? `${e.syncArtist} – ${e.syncTitle}` : e.syncTitle))
    try {
      await navigator.clipboard.writeText(lines.join('\n'))
      toast(`Copied ${missing.length} ${missing.length === 1 ? 'song' : 'songs'} you don’t have`, ClipboardList)
    } catch {
      toast('Couldn’t copy')
    }
  }
  async function updateFromSpotify(p: Playlist) {
    let updated: boolean
    try {
      updated = await readSpotify(p.id, true)
    } catch (e) {
      return toast(`Couldn’t read it on Spotify: ${e}`, RefreshCw)
    }
    // A long one (over 100 songs) that changed there: Spotify shares only its first 100, the rest come from a CSV.
    const now = await library.playlist(p.id).catch(() => null)
    if (now?.spotify?.changed) spotifyUpdate.value = now
    else toast(updated ? 'Updated from Spotify' : 'Up to date with Spotify', RefreshCw)
  }
  async function copyMissingLinks(p: Playlist) {
    const [entries, links] = await Promise.all([library.playlistEntries(p.id), library.spotifyLinks(p.id)])
    const missing = entries.filter((e) => !e.track && e.syncTitle)
    if (!missing.length) return toast('You have all its songs', Link2)
    // Songs read before Melue kept their ids have none until the playlist's next update.
    const found = missing.flatMap((e) => (e.fileEntry && links[e.fileEntry] ? [links[e.fileEntry]] : []))
    if (!found.length) return toast('No Spotify links yet: they come with the playlist’s next update', Link2)
    const without = missing.length - found.length
    try {
      await navigator.clipboard.writeText(found.join('\n'))
      const count = `${found.length} ${found.length === 1 ? 'link' : 'links'}${without ? ` (${without} without one yet)` : ''}`
      toast(`Copied ${count}. In Spotify on a computer, open a playlist and press Ctrl+V`, Link2)
    } catch {
      toast('Couldn’t copy')
    }
  }
  async function saveCover(p: Playlist, picture: Uint8Array) {
    await library.setPlaylistCover(p.id, picture)
    await changed()
  }
  return { renaming, asking, covering, spotifyUpdate, act, rename, confirm, saveCover }
}
