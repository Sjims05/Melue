/**
 * What your playlists' menus do, the same on the Playlists page and a playlist's own page: play it (and the rest of
 * the playing items), rename it (its name turns into a text field: `renaming`), keep it only here or sync it, stop
 * following its file or Spotify (asked first) or follow it again, delete it (asked first), copy a Spotify playlist's
 * songs you don't have ("Artist – Title" lines), and the About items (`asking`, shown by PlaylistDialogs).
 */
import { ClipboardList } from '@lucide/vue'
import { ref } from 'vue'
import type { CoverAction } from '@/components/covers'
import { library, type Playlist } from '@/library'
import { librarySync } from '@/library/sync'
import { toast } from '@/lib/toast'
import { exportList, listAction } from '@/player/songs'

/** A question or explanation about a playlist: delete it? stop following its file? about how it lives. */
export type PlaylistAsk = { what: 'delete' | 'unlink' | 'about'; playlist: Playlist }

export function usePlaylistActions(changed: () => unknown, deleted?: () => unknown) {
  const renaming = ref<number | null>(null)
  const asking = ref<PlaylistAsk | null>(null)
  // Change cover: the playlist whose cover is being chosen (the crop window).
  const covering = ref<Playlist | null>(null)

  async function act(p: Playlist, action: CoverAction) {
    if (action === 'rename') renaming.value = p.id
    else if (action === 'delete' || action === 'unlink' || action === 'about') asking.value = { what: action, playlist: p }
    else if (action === 'keepHereOnly' || action === 'syncEverywhere') {
      await librarySync.setPlaylistLocalOnly(p.id, action === 'keepHereOnly')
      await changed()
    } else if (action === 'changeCover') {
      covering.value = p
    } else if (action === 'revertCover') {
      await library.clearPlaylistCover(p.id)
      await changed()
    } else if (action === 'export') {
      await exportList(`playlist:${p.id}`, p.name)
    } else if (action === 'copyMissing') {
      await copyMissing(p)
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
  async function confirm(ask: PlaylistAsk) {
    if (ask.what === 'delete') {
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
  async function saveCover(p: Playlist, picture: Uint8Array) {
    await library.setPlaylistCover(p.id, picture)
    await changed()
  }
  return { renaming, asking, covering, act, rename, confirm, saveCover }
}
