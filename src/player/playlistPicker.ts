/**
 * The song menus' Add to playlist, for the whole app (DesktopShell provides it): your playlists by name with a ✓ on
 * the ones that have the song, adding it to one, or a new playlist with it (opened, its name ready to type).
 */
import type { PlaylistPicker } from '@/components/songs'
import { library } from '@/library'
import { router } from '@/router'

export const playlistPicker: PlaylistPicker = {
  async list(song) {
    const [playlists, has] = await Promise.all([library.playlists(), library.playlistsWithTrack(Number(song.id))])
    return playlists.map((p) => ({ id: p.id, name: p.name, has: has.includes(p.id) }))
  },
  async add(song, playlist) {
    await library.addToPlaylist(playlist, [Number(song.id)])
  },
  async create(song) {
    const made = await library.createPlaylist('New playlist')
    await library.addToPlaylist(made.id, [Number(song.id)])
    router.push({ path: `/playlist/${made.id}`, query: { rename: '1' } })
  },
}
