/**
 * The song menus' Add to playlist, for the whole app (DesktopShell provides it): your playlists by name with a ✓ on
 * the ones that have the song, adding it to one, or a new playlist with it (opened, its name ready to type). Adding
 * says how it went: a note with Undo (tapped: the playlist, the song pointed out), or why it couldn't.
 */
import type { PlaylistPicker } from '@/components/songs'
import { ListPlus } from '@lucide/vue'
import { library } from '@/library'
import { pointOut } from '@/lib/spotlight'
import { toast } from '@/lib/toast'
import { router } from '@/router'

export const playlistPicker: PlaylistPicker = {
  async list(song) {
    const [playlists, has] = await Promise.all([library.playlists(), library.playlistsWithTrack(Number(song.id))])
    return playlists.map((p) => ({ id: p.id, name: p.name, has: has.includes(p.id) }))
  },
  async add(song, playlist) {
    try {
      const [entry] = await library.addToPlaylist(playlist, [Number(song.id)])
      const name = await library.playlist(playlist).then((p) => p.name, () => 'the playlist')
      toast(
        `Added “${song.title}” to “${name}”`,
        ListPlus,
        { label: 'Undo', run: () => library.removeFromPlaylist(playlist, [entry]).catch((e) => toast(`Couldn’t take it out again: ${e}`)) },
        () => {
          router.push(`/playlist/${playlist}`)
          pointOut((songs) => songs.find((s) => s.key === `e${entry}`))
        },
      )
    } catch (e) {
      toast(`Couldn’t add “${song.title}”: ${e}`)
    }
  },
  async create(song) {
    const made = await library.createPlaylist('New playlist')
    await library.addToPlaylist(made.id, [Number(song.id)])
    router.push({ path: `/playlist/${made.id}`, query: { rename: '1' } })
  },
}
