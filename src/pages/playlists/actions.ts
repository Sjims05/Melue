/**
 * What your playlists' menus do, the same on the Playlists page and a playlist's own page: play it (and the rest of
 * the playing items), rename it (its name turns into a text field: `renaming`), keep it only here or sync it, stop
 * following its file (asked first) or follow it again, delete it (asked first), and the About items (`asking`, shown
 * by PlaylistDialogs).
 */
import { ref } from 'vue'
import type { CoverAction } from '@/components/covers'
import { library, type Playlist } from '@/library'
import { librarySync } from '@/library/sync'
import { exportList, listAction } from '@/player/songs'

/** A question or explanation about a playlist: delete it? stop following its file? about how it lives. */
export type PlaylistAsk = { what: 'delete' | 'unlink' | 'about'; playlist: Playlist }

export function usePlaylistActions(changed: () => unknown, deleted?: () => unknown) {
  const renaming = ref<number | null>(null)
  const asking = ref<PlaylistAsk | null>(null)

  async function act(p: Playlist, action: CoverAction) {
    if (action === 'rename') renaming.value = p.id
    else if (action === 'delete' || action === 'unlink' || action === 'about') asking.value = { what: action, playlist: p }
    else if (action === 'keepHereOnly' || action === 'syncEverywhere') {
      await librarySync.setPlaylistLocalOnly(p.id, action === 'keepHereOnly')
      await changed()
    } else if (action === 'export') {
      await exportList(`playlist:${p.id}`, p.name)
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
      await library.setPlaylistFollowing(ask.playlist.id, false)
      await changed()
    }
  }
  return { renaming, asking, act, rename, confirm }
}
