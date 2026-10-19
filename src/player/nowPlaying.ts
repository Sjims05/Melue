/**
 * What's playing, for every view of the player (the player bar, Now playing, the song lists' playing song): the
 * player's state, its queue with the songs' details (lengths, likes, covers, from the library), the list it plays
 * from in words, and the position, moving smoothly between the player's updates. And the controls.
 *
 * Up next is the queue after the playing song, in the order it plays.
 */
import { computed, reactive } from 'vue'
import type { RouteLocationRaw } from 'vue-router'
import { artistParts, type Song } from '@/components/songs'
import { coverUrl, library, type Track } from '@/library'
import { onlyHere } from '@/library/onlyHere'
import { setLiked } from '@/library/playback'
import { playbackSync } from '@/library/sync'
import { player, type PlayerQueue, type PlayerState, type PlayerTrack, type RepeatMode } from '@/platform'

export const nowPlaying = reactive({
  state: null as PlayerState | null,
  /** When `state` came (ms): the position moves on from there while playing. */
  stateAt: 0,
  queue: null as PlayerQueue | null,
  /** The queue's songs from the library (by id). */
  tracks: new Map<number, Track>(),
  liked: new Set<number>(),
  /** Where the queue plays from, in words ("Liked songs", the album's title). */
  from: '',
  /** Ticks while playing, so the position (and the seek bar) moves. */
  tick: 0,
})

let started = false
let lastQueueKey = ''

/** Once (the app's frame): follows the player, its queue and the likes. */
export async function startNowPlaying() {
  if (started) return
  started = true
  nowPlaying.liked = new Set(await library.likedIds().catch(() => []))
  await library.onChange(async (change) => {
    if (change.reason === 'likes') nowPlaying.liked = new Set(await library.likedIds().catch(() => []))
  })
  await player.onState(applyState)
  // Every frame while playing, also with the window in the background (on a second screen: WebView2 is told not to
  // pause it, tauri.windows.conf.json); not while the window is hidden (in the tray), where nothing shows.
  const frame = () => {
    if (nowPlaying.state?.isPlaying && !document.hidden) nowPlaying.tick++
    requestAnimationFrame(frame)
  }
  requestAnimationFrame(frame)
  // Another device's playback loaded here (synced playback, following it): what's playing now.
  playbackSync.onFollowed(() => syncNowPlaying().catch(() => {})).catch(() => {})
  // Back from the background (the phone woken, the window shown): what's playing, at once.
  document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && syncNowPlaying().catch(() => {}))
}

/** The player's state, from its updates or a control's answer. */
function applyState(state: PlayerState) {
  // The same song with no length for a moment (Android's player getting ready again, when the queue changes during a
  // sync): its length as known, or the progress bar would show it played to the end until the length is back.
  const before = nowPlaying.state
  if (!(state.duration > 0) && before && before.currentId === state.currentId && before.duration > 0) state = { ...state, duration: before.duration }
  nowPlaying.state = state
  nowPlaying.stateAt = performance.now()
  // The queue changed (another song, added, removed, moved, shuffled, or repeat, which decides the next pass): read it
  // again.
  const key = `${state.queueLength}:${state.queueIndex}:${state.currentId}:${state.shuffle}:${state.repeatMode}`
  // Or what's shown isn't what the player has (it was read before the player resumed the last queue at start).
  const shown = nowPlaying.queue
  const stale =
    (shown?.items.length ?? 0) !== state.queueLength ||
    (state.queueIndex >= 0 && shown?.items[shown.currentIndex]?.id !== state.currentId)
  // While Up next is changed in several steps (Clear), not read half-done: once, at the end (holdingQueue).
  if (holding) return
  if (key !== lastQueueKey || (stale && !refreshing)) {
    lastQueueKey = key
    refreshQueue()
  }
}

/** Reads the player again: after it has resumed the last queue at start (a queue resumed paused sends nothing). */
export async function syncNowPlaying() {
  applyState(await player.getState())
}

let holding = 0
/** Makes several changes to the queue, then reads it once (what's shown doesn't jump back between them). */
async function holdingQueue(change: () => Promise<unknown>) {
  holding++
  try {
    await change()
  } finally {
    holding--
  }
  const state = await player.getState()
  lastQueueKey = `${state.queueLength}:${state.queueIndex}:${state.currentId}:${state.shuffle}:${state.repeatMode}`
  nowPlaying.state = state
  nowPlaying.stateAt = performance.now()
  await refreshQueue()
}

let refreshing = false
export async function refreshQueue() {
  refreshing = true
  const queue = await player.getQueue().catch(() => null)
  refreshing = false
  nowPlaying.queue = queue
  if (!queue) return
  const missing = [...new Set(queue.items.map((i) => i.id))].filter((id) => !nowPlaying.tracks.has(id))
  if (missing.length) for (const t of await library.tracksByIds(missing).catch(() => [])) nowPlaying.tracks.set(t.id, t)
  nowPlaying.from = await fromWords(queue)
}

const TOP: Record<string, string> = { month: 'last month', 'half-year': 'last 6 months', year: 'last year', 'all-time': 'all time' }
async function fromWords(queue: PlayerQueue): Promise<string> {
  const [kind, id] = (queue.sourceId ?? '').split(':')
  const playing = queue.items[queue.currentIndex]
  if (kind === 'liked') return 'Liked songs'
  if (kind === 'songs') return 'All songs'
  if (kind === 'top') return `Top songs: ${TOP[id] ?? id}`
  if (kind === 'album') return playing?.album ?? 'an album'
  if (kind === 'artist') return playing?.artist ?? 'an artist'
  if (kind === 'playlist') return (await library.playlist(Number(id)).catch(() => null))?.name ?? 'a playlist'
  if (kind === 'folder') return (await library.folders().catch(() => [])).find((f) => f.id === Number(id))?.name ?? 'a folder'
  return 'your queue'
}

/** A queue item as a song row shows it. */
export function songOf(item: PlayerTrack): Song {
  const t = nowPlaying.tracks.get(item.id)
  return {
    id: item.id,
    title: t?.title ?? item.title ?? 'Unknown',
    artist: t?.artist ?? item.artist ?? '',
    album: t?.album ?? item.album ?? undefined,
    duration: (t?.durationMs ?? 0) / 1000,
    cover: t?.cover ? coverUrl(t.cover) : undefined,
    liked: nowPlaying.liked.has(item.id),
    localOnly: onlyHere.value.has(item.id),
    artists: t?.artists,
    albumId: t?.albumId,
  }
}

// ---- What the views show

export const current = computed(() => {
  const q = nowPlaying.queue
  const item = q && q.currentIndex >= 0 ? q.items[q.currentIndex] : null
  return item ? songOf(item) : null
})
/** The page of the list the queue plays from ("Playing from …" links to it), or null. */
export const fromPage = computed((): RouteLocationRaw | null => {
  const [kind, id] = (nowPlaying.queue?.sourceId ?? '').split(':')
  // All songs from the Songs tab (in its order: 'songs:plays') is the tab; All songs from Made for you, its page.
  if (kind === 'songs') return id ? { path: '/library', query: { tab: 'songs' } } : '/playlist/songs'
  if (kind === 'liked') return '/playlist/liked'
  if (kind === 'top') return `/playlist/top-${id}`
  if (['album', 'artist', 'playlist', 'folder'].includes(kind) && id) return `/${kind}/${id}`
  return null
})

/** The playing song's artist line in parts, each artist's name linked to their page (see artistParts). */
export const currentArtistParts = computed(() => artistParts(current.value?.artist ?? '', current.value?.artists))

/** The big cover (1000 px) of the playing song. */
export const currentCoverLarge = computed(() => {
  const id = current.value?.id
  const cover = id ? nowPlaying.tracks.get(Number(id))?.cover : null
  return cover ? coverUrl(cover, 'large') : undefined
})
/** Seconds into the song, moving on while it plays. */
export const position = computed(() => {
  const s = nowPlaying.state
  if (!s) return 0
  void nowPlaying.tick
  // Run on from the player's last word, a few seconds at most: after a while away (the app in the background) it would
  // reach the song's end before the player's next word comes.
  const moved = s.isPlaying ? Math.min(5, (performance.now() - nowPlaying.stateAt) / 1000) : 0
  return Math.min(s.duration || Infinity, s.currentTime + moved)
})
/** Up next: the queue after the playing song, in play order, with each one's index in the queue's items. */
export const upNext = computed(() => {
  const q = nowPlaying.queue
  if (!q || q.currentIndex < 0) return []
  const order = q.playOrder.length ? q.playOrder : q.items.map((_, i) => i)
  return keyed(order.slice(order.indexOf(q.currentIndex) + 1).map((index) => ({ index, song: songOf(q.items[index]) })))
})
/**
 * Up next as shown: with repeat all, the whole next pass follows, greyed (it can't be moved or taken out there), in the
 * order it will play: with shuffle the player has worked it out ahead (it's shuffled again each time), else the list.
 */
export const upNextShown = computed(() => {
  const q = nowPlaying.queue
  if (!q || q.currentIndex < 0 || nowPlaying.state?.repeatMode !== 'all') return upNext.value
  const order = q.nextPassOrder ?? q.items.map((_, i) => i)
  const again = order.map((index) => ({
    index,
    song: { ...songOf(q.items[index]), key: `again:${index}`, later: 'After the queue starts over' },
  }))
  return [...upNext.value, ...again]
})
// A key for each row that stays with it when songs move: the song's id, and which time it is in the list (a song can
// be in the queue twice).
function keyed<T extends { song: Song }>(rows: T[]): T[] {
  const seen = new Map<Song['id'], number>()
  return rows.map((row) => {
    const n = (seen.get(row.song.id) ?? 0) + 1
    seen.set(row.song.id, n)
    return { ...row, song: { ...row.song, key: `${row.song.id}#${n}` } }
  })
}

// ---- The controls

// Each control's answer is the player's new state: shown at once (a button lights up as it's pressed), not only when
// the player's next update comes.
const shown = (answer: Promise<PlayerState>) => answer.then(applyState)
export const togglePlay = () => shown(nowPlaying.state?.isPlaying ? player.pause() : player.play())
export const next = () => shown(player.next())
export const previous = () => shown(player.previous())
export const seek = (seconds: number) => shown(player.seekTo(seconds))
export const setVolume = (volume: number) => shown(player.setVolume(volume))
export const toggleShuffle = () => shown(player.setShuffle(!nowPlaying.state?.shuffle))
export const setShuffle = (on: boolean) => shown(player.setShuffle(on))
const REPEAT_NEXT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' }
export const cycleRepeat = () => shown(player.setRepeatMode(REPEAT_NEXT[nowPlaying.state?.repeatMode ?? 'off']))
/** The player bar's ✕: stops and empties the player (the bar stays, empty). */
export async function clearPlayer() {
  await player.dispose()
  applyState(await player.getState())
  await refreshQueue()
}
export async function like(id: number, liked: boolean) {
  if (liked) nowPlaying.liked.add(id)
  else nowPlaying.liked.delete(id)
  await setLiked(id, liked)
}
/**
 * Up next: play this one now. It moves to the front of Up next and plays from there (like Play next, then skip): the
 * rest of Up next stays as it was, and Previous goes back to the song that was playing. (Skipping to it would leave
 * the songs before it behind, and with shuffle on, shuffle everything after it again.)
 */
export async function playFromUpNext(index: number) {
  const at = upNext.value.findIndex((u) => u.index === index)
  if (at < 0) return
  if (at > 0) await moveInUpNext(at, 0)
  await shown(player.next())
  if (!nowPlaying.state?.isPlaying) await shown(player.play())
}
/** Up next: move a song (places in Up next: in the play order, so it works with shuffle too), take one out, or empty it. */
export async function moveInUpNext(from: number, to: number) {
  const q = nowPlaying.queue
  if (!q) return
  const order = q.playOrder.length ? q.playOrder : q.items.map((_, i) => i)
  // Up next starts right after the playing song's place in the play order.
  const start = order.indexOf(q.currentIndex) + 1
  // Shown moved at once (the song glides into its place as it's let go), not only once the player has answered.
  const moved = [...order]
  moved.splice(start + to, 0, ...moved.splice(start + from, 1))
  if (nowPlaying.state?.shuffle) nowPlaying.queue = { ...q, playOrder: moved }
  else {
    // Without shuffle the play order is the list: the list itself changes.
    const items = moved.map((i) => q.items[i])
    nowPlaying.queue = { ...q, items, currentIndex: items.indexOf(q.items[q.currentIndex]), playOrder: items.map((_, i) => i) }
  }
  await player.moveInPlayOrder(start + from, start + to)
  await refreshQueue()
}
export async function removeFromUpNext(at: number) {
  const { index } = upNext.value[at]
  takeOut([index])
  await player.removeFromQueue(index)
  await refreshQueue()
}
/** Takes these queue items out (by their place in the queue's list), shown at once and read back once at the end. */
export async function takeOutOfQueue(indexes: number[]) {
  if (!indexes.length) return
  takeOut(indexes)
  await holdingQueue(async () => {
    for (const index of [...indexes].sort((a, b) => b - a)) await player.removeFromQueue(index)
  })
}
export async function clearUpNext() {
  const indexes = upNext.value.map((u) => u.index)
  takeOut(indexes)
  // From the end, so the indexes before stay right.
  await holdingQueue(async () => {
    for (const index of indexes.sort((a, b) => b - a)) await player.removeFromQueue(index)
  })
}
// Taken out of the queue as shown at once (the list closes up as the songs fold away), not only once the player has
// answered: the items go, and the indexes after them move down.
function takeOut(indexes: number[]) {
  const q = nowPlaying.queue
  if (!q) return
  const gone = new Set(indexes)
  const moved = (i: number) => i - indexes.filter((g) => g < i).length
  const keep = (order: number[]) => order.filter((i) => !gone.has(i)).map(moved)
  nowPlaying.queue = {
    ...q,
    items: q.items.filter((_, i) => !gone.has(i)),
    currentIndex: moved(q.currentIndex),
    playOrder: keep(q.playOrder),
    nextPassOrder: q.nextPassOrder && keep(q.nextPassOrder),
  }
}
