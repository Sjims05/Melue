/**
 * Songs from the library in the app's lists: a library song as a song row shows it, and what a song's menu does
 * (Play next, Add to queue, like, Go to album / artist, Show in folder), the same in every list. And what a whole
 * list's buttons and card menu do (an album's Play and Shuffle; Play next, Add to queue, Go to artist).
 */
import { CloudUpload, ListEnd, ListStart, MonitorSpeaker } from '@lucide/vue'
import { audioDir, join } from '@tauri-apps/api/path'
import { save } from '@tauri-apps/plugin-dialog'
import type { CoverAction } from '@/components/covers'
import type { Song, SongAction } from '@/components/songs'
import { coverUrl, library, type QueueItem, type Source, type Track } from '@/library'
import { onNetwork, onlyHere } from '@/library/onlyHere'
import { playSource } from '@/library/playback'
import { describeSyncFailure, moving } from '@/library/sync'
import { ask } from '@/lib/confirm'
import { count } from '@/lib/format'
import { whereOriginalsGo } from '@/settings/format'
import { listPlay } from '@/lib/listPlay'
import { pointOut } from '@/lib/spotlight'
import { toast } from '@/lib/toast'
import { libraryState } from '@/settings/libraryState'
import { remote } from '@/library/remote'
import { playingOn } from './playingOn'
import { isMobile, player } from '@/platform'
import { router } from '@/router'
import { current, like, next, nowPlaying, refreshQueue, setShuffle, takeOutOfQueue, togglePlay } from './nowPlaying'

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
    localOnly: onlyHere.value.has(t.id),
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
  const file = `${name.replace(/[\\/:*?"<>|]/g, '').trim() || 'Playlist'}.m3u8`
  const path = await save({ defaultPath: await join(await audioDir().catch(() => ''), file), filters: [{ name: 'Playlist', extensions: ['m3u8'] }] })
  if (!path) return null
  await library.exportPlaylist(source, path)
  return path
}

/**
 * One song (its play button): it plays and Up next stays after it; nothing else from its list is lined up. Queue
 * settings: it plays now (the playing song ends) or next. Nothing playing: it plays on its own.
 */
export async function playOne(id: number) {
  if (id === current.value?.id) return togglePlay()
  // Playing on another device: the song plays there (on its own).
  if (playingOn.device) return remote.load([id], 0).catch((e) => toast(String(e), MonitorSpeaker))
  const items = await library.queueItemsFor([id])
  if (!current.value) {
    await player.setQueue(items, { startIndex: 0 })
    await player.play()
    return
  }
  const now = listPlay.songPlays === 'now'
  await addItems(items, true, !now)
  if (now) {
    await next()
    if (!nowPlaying.state?.isPlaying) await player.play()
  }
  await refreshQueue()
}

/**
 * A song's own play in any list (not the list's big Play): its play button plays just it (playOne), a double-click
 * adds it to the end of Up next (queueOne). The song playing (or paused on) is paused or resumed.
 */
export const playSong = (song: Song, via: 'button' | 'doubleClick' = 'button') => (via === 'doubleClick' ? queueOne : playOne)(Number(song.id))

/** One song (a double-click): to the end of Up next; nothing playing: it plays. */
export async function queueOne(id: number) {
  if (id === current.value?.id) return togglePlay()
  if (playingOn.device) return addThere([id], null, false)
  if (!current.value) return playOne(id)
  await addItems(await library.queueItemsFor([id]), false)
  await refreshQueue()
}

/**
 * Adds to the queue: right after the playing song (`playNext`) or at the end. `note`: says what went in ("“Song” plays
 * next", "Added 12 songs to the queue"), with Undo.
 */
async function addItems(items: QueueItem[], playNext: boolean, note = true) {
  if (!items.length) return
  if (playingOn.device) return addThere(items.map((i) => Number(i.id)), null, playNext)
  // Where they go in the queue's list, for Undo.
  const before = await player.getQueue().catch(() => null)
  const at = playNext ? (before?.currentIndex ?? -1) + 1 : (before?.items.length ?? 0)
  await player.addToQueue(items, { playNext })
  if (!note) return
  const what = items.length === 1 ? `“${items[0].title ?? 'The song'}”` : count(items.length, 'song')
  const undo = { label: 'Undo', run: () => undoAdd(items.map((i) => i.id), at) }
  // Tapped: Up next, with the (first) song that went in pointed out: playing next, the first of it there; at the end,
  // the last.
  const first = String(items[0].id)
  const show = () => {
    router.push({ path: '/now-playing', query: { queue: 'open' } })
    pointOut((songs) => (playNext ? songs.find((s) => String(s.id) === first) : [...songs].reverse().find((s) => String(s.id) === first && !s.later)), 750)
  }
  if (playNext) toast(`${what} ${items.length === 1 ? 'plays' : 'play'} next`, ListStart, undo, show)
  else toast(`Added ${what} to the queue`, ListEnd, undo, show)
}

/**
 * Playing on another device: songs (ids here) or a list into Up next there. Songs only on this device can't go (left
 * out, said so); the list there is that device's own copy.
 */
async function addThere(ids: number[], source: Source | null, playNext: boolean) {
  const name = playingOn.device?.name ?? 'the other device'
  try {
    const left = await remote.add(ids, source, playNext)
    const what = source ? 'The list' : ids.length - left === 1 ? 'The song' : count(ids.length - left, 'song')
    const verb = source || ids.length - left === 1 ? (playNext ? 'plays' : 'goes') : playNext ? 'play' : 'go'
    toast(`${what} ${verb} ${playNext ? 'next' : 'into the queue'} on ${name}`, playNext ? ListStart : ListEnd)
    // Tapped: the list of them in Settings, to put them on the share.
    const showThem = () => {
      libraryState.listToOpen = 'onlyHere'
      router.push({ path: '/settings', query: { setting: 'sync.onlyHere' } })
    }
    if (left) toast(`${count(left, 'song')} only on this device ${left === 1 ? 'was' : 'were'} left out`, MonitorSpeaker, undefined, showThem)
  } catch (e) {
    toast(String(e), MonitorSpeaker)
  }
}

/**
 * Undo: takes the songs just added out again: from where they went, or (the queue changed since) their last places in
 * it. Not the song playing (it may be one of them by now).
 */
async function undoAdd(ids: number[], at: number) {
  await refreshQueue()
  const q = nowPlaying.queue
  if (!q) return
  let indexes = ids.map((_, i) => at + i)
  if (!ids.every((id, i) => q.items[at + i]?.id === id)) {
    const taken = new Set<number>()
    indexes = ids.map((id) => {
      for (let i = q.items.length - 1; i >= 0; i--) {
        if (q.items[i].id === id && !taken.has(i)) {
          taken.add(i)
          return i
        }
      }
      return -1
    })
  }
  await takeOutOfQueue(indexes.filter((i) => i >= 0 && i !== q.currentIndex))
  toast(ids.length === 1 ? 'Taken out of the queue again' : `${count(ids.length, 'song')} taken out of the queue again`, ListEnd)
}

/** Does what a list's card menu item (or button) says; `artistId`: its artist's page (an album's Go to artist). */
export async function listAction(source: Source, action: CoverAction, artistId?: number | null) {
  switch (action) {
    case 'play':
    case 'shuffle':
      await playList(source, { shuffle: action === 'shuffle' })
      break
    case 'playNext':
    case 'addToQueue': {
      if (playingOn.device) return addThere([], source, action === 'playNext')
      await addItems(await library.queueItems(source), action === 'playNext')
      await refreshQueue()
      break
    }
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
    case 'addToQueue': {
      await addItems(await library.queueItemsFor([id]), action === 'playNext')
      await refreshQueue()
      break
    }
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
    case 'moveToSync':
      await moveToSync(song)
      break
  }
}

/**
 * Move to sync (a song only on this device): asked first, since the original goes (the Recycle Bin, or for good from a
 * network folder; on Android its trash, after Android asks). It's copied into the synced music, then the original is removed; a note says how it went.
 */
async function moveToSync(song: Song) {
  const yes = await ask({
    title: `Move “${song.title}” to sync?`,
    text: `It’s copied into your synced music first, so every device gets it. Then the original ${whereOriginalsGo(
      isMobile,
      onNetwork.value.has(Number(song.id)),
      false,
    )}`,
    action: 'Move',
  })
  if (!yes) return
  toast(`Moving “${song.title}” to sync…`, CloudUpload)
  try {
    const r = await moving.moveToSync([{ kind: 'song', id: Number(song.id) }])
    if (r.failed.length) toast(`Couldn’t move “${song.title}”: ${r.failed[0][1]}`)
    else toast(`Moved “${song.title}” to sync`, CloudUpload)
  } catch (e) {
    toast(describeSyncFailure(e))
  }
}
