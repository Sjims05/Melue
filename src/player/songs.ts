/**
 * Songs from the library in the app's lists: a library song as a song row shows it, and what a song's menu does
 * (Play next, Add to queue, like, Go to album / artist, Show in folder), the same in every list. And what a whole
 * list's buttons and card menu do (an album's Play and Shuffle; Play next, Add to queue, Go to artist).
 */
import type { CoverAction } from '@/components/covers'
import type { Song, SongAction } from '@/components/songs'
import { coverUrl, library, type Source, type Track } from '@/library'
import { playSource } from '@/library/playback'
import { listPlay } from '@/lib/listPlay'
import { player } from '@/platform'
import { router } from '@/router'
import { like, nowPlaying, refreshQueue, setShuffle } from './nowPlaying'

/** A library song as a song row shows it (liked as the player knows it, so a like shows everywhere at once). */
export function trackSong(t: Track): Song {
  return {
    id: t.id,
    title: t.title,
    artist: t.artist ?? '',
    album: t.album ?? undefined,
    albumId: t.albumId,
    duration: (t.durationMs ?? 0) / 1000,
    cover: t.cover ? coverUrl(t.cover) : undefined,
    liked: nowPlaying.liked.has(t.id),
    artists: t.artists,
    plays: t.playCount,
    addedAt: t.addedAt,
  }
}

/**
 * Plays a list: its Play (from its first song; with shuffle on, Queue settings decide: shuffle off first, or as it is)
 * or Shuffle (shuffle on, from a random one of its songs). `startAt`: from that song (a double-click on it), shuffle
 * left as it is.
 */
export async function playList(source: Source, options: { shuffle?: boolean; startAt?: number } = {}) {
  if (options.shuffle) await setShuffle(true)
  else if (options.startAt == null && listPlay.playFromStart && nowPlaying.state?.shuffle) await setShuffle(false)
  await playSource(source, { startAt: options.startAt })
}

/**
 * Exports a list as an .m3u8 (Save as, in your Music folder, named after it); the file has each song's full path, its
 * title, artist and length. Resolves with where it went (nothing when cancelled).
 */
export async function exportList(source: Source, name: string): Promise<string | null> {
  const { save } = await import('@tauri-apps/plugin-dialog')
  const { audioDir, join } = await import('@tauri-apps/api/path')
  const file = `${name.replace(/[\\/:*?"<>|]/g, '').trim() || 'Playlist'}.m3u8`
  const path = await save({ defaultPath: await join(await audioDir().catch(() => ''), file), filters: [{ name: 'Playlist', extensions: ['m3u8'] }] })
  if (!path) return null
  await library.exportPlaylist(source, path)
  return path
}

/** Does what a list's card menu item (or button) says; `artistId`: its artist's page (an album's Go to artist). */
export async function listAction(source: Source, action: CoverAction, artistId?: number | null) {
  switch (action) {
    case 'play':
    case 'shuffle':
      await playList(source, { shuffle: action === 'shuffle' })
      break
    case 'playNext':
    case 'addToQueue':
      await player.addToQueue(await library.queueItems(source), { playNext: action === 'playNext' })
      await refreshQueue()
      break
    case 'goToArtist':
      if (artistId != null) router.push(`/artist/${artistId}`)
      break
  }
}

/** Does what a song's menu item (or its like button) says. */
export async function songAction(song: Song, action: SongAction) {
  const id = Number(song.id)
  switch (action) {
    case 'playNext':
    case 'addToQueue':
      await player.addToQueue(await library.queueItemsFor([id]), { playNext: action === 'playNext' })
      await refreshQueue()
      break
    case 'like':
      await like(id, !song.liked)
      break
    case 'goToAlbum':
      if (song.albumId != null) router.push(`/album/${song.albumId}`)
      break
    case 'goToArtist':
      if (song.artists?.length) router.push(`/artist/${song.artists[0].id}`)
      break
    case 'showInFolder':
      await library.showInFolder(id)
      break
  }
}
