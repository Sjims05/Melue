/**
 * Songs from the library in the app's lists: a library song as a song row shows it, and what a song's menu does
 * (Play next, Add to queue, like, Go to album / artist, Show in folder), the same in every list. And what a whole
 * list's buttons and card menu do (an album's Play and Shuffle; Play next, Add to queue, Go to artist).
 */
import type { CoverAction } from '@/components/covers'
import type { Song, SongAction } from '@/components/songs'
import { coverUrl, library, type Source, type Track } from '@/library'
import { playSource } from '@/library/playback'
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
 * Plays a list: from its start, or (shuffle) with shuffle on, from a random one of its songs. `startAt`: from that
 * song (a double-click on it).
 */
export async function playList(source: Source, options: { shuffle?: boolean; startAt?: number } = {}) {
  if (options.shuffle) await setShuffle(true)
  await playSource(source, { startAt: options.startAt })
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
