/**
 * What's playing, for every view of the player (the player bar, Now playing, the song lists' playing song): the
 * player's state, its queue with the songs' details (lengths, likes, covers, from the library), the list it plays
 * from in words, and the position, moving smoothly between the player's updates. And the controls.
 *
 * Up next is the queue after the playing song, in the order it plays.
 *
 * Playing on another device (Connect device: playingOn.ts): all of it is that device's, its state as it says it (its
 * songs this device has get their details from the library here), and the controls go to it. Changing Up next itself
 * (moving, taking out, adding) is done on that device for now.
 */
import { CircleAlert, MonitorSpeaker } from '@lucide/vue'
import { computed, reactive, shallowRef, watch } from 'vue'
import type { RouteLocationRaw } from 'vue-router'
import { artistParts, type Song } from '@/components/songs'
import { listen } from '@tauri-apps/api/event'
import { coverUrl, library, type Track } from '@/library'
import { onlyHere } from '@/library/onlyHere'
import { setLiked } from '@/library/playback'
import { toast } from '@/lib/toast'
import { playbackSync, type StandIn } from '@/library/sync'
import { player, type PlayerQueue, type PlayerState, type PlayerTrack, type RepeatMode } from '@/platform'
import { remote, type RemoteCommand, type RemoteState } from '@/library/remote'
import { playingOn } from './playingOn'

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
  // Continued from another device whose song isn't here: that song shows until this one plays.
  followStandIn().catch(() => {})
  // Another device playing here changed the player (Connect device): read again, at once and once it has settled (a
  // queue just loaded takes a moment to start).
  listen('remote-changed-here', () => {
    const read = () => syncNowPlaying().then(refreshQueue).catch(() => {})
    read()
    setTimeout(read, 700)
  }).catch(() => {})
  // Back from the background (the phone woken, the window shown): what's playing, at once.
  document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && syncNowPlaying().catch(() => {}))
}

/** The player's state, from its updates or a control's answer. */
function applyState(state: PlayerState) {
  // Playing on another device: its state is what's shown (this device's player is paused).
  if (playingOn.device) return
  // The same song with no length for a moment (Android's player getting ready again, when the queue changes during a
  // sync): its length as known, or the progress bar would show it played to the end until the length is back.
  const before = nowPlaying.state
  if (!(state.duration > 0) && before && before.currentId === state.currentId && before.duration > 0) state = { ...state, duration: before.duration }
  // A song that can't play (its file gone, a format the player can't read): a note says which, and why.
  if (state.status === 'error' && before?.status !== 'error') {
    const title = nowPlaying.queue?.items.find((i) => i.id === state.currentId)?.title ?? 'This song'
    toast(`Can’t play “${title}”${state.error ? `: ${state.error}` : ''}`, CircleAlert)
  }
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
  if (playingOn.device) return
  applyState(await player.getState())
}

// ---- Playing on another device

/** The other device's songs: their ids here, by path on the share (asked once per song). */
const idsHere = new Map<string, number | null>()

/** The other device's state, as this device's views show a player: its songs that are here as themselves. */
async function applyRemote(s: RemoteState) {
  const unknown = [...new Set(s.queue.map((q) => q.path).filter((p): p is string => !!p && !idsHere.has(p)))]
  if (unknown.length) {
    const ids = await remote.songIds(unknown).catch(() => unknown.map(() => null))
    unknown.forEach((path, i) => idsHere.set(path, ids[i] ?? null))
  }
  // A song that isn't here: a stand-in id (negative), with what the other device says it is.
  const items: PlayerTrack[] = s.queue.map((q, i) => ({ id: (q.path && idsHere.get(q.path)) || -(i + 1), src: '', title: q.title ?? undefined, artist: q.artist ?? undefined }))
  const missing = [...new Set(items.map((i) => i.id))].filter((id) => id > 0 && !nowPlaying.tracks.has(id))
  if (missing.length) for (const t of await library.tracksByIds(missing).catch(() => [])) nowPlaying.tracks.set(t.id, t)
  if (!playingOn.device) return
  nowPlaying.queue = { items, currentIndex: s.index, playOrder: s.playOrder.length ? s.playOrder : items.map((_, i) => i), sourceId: null }
  nowPlaying.state = {
    status: s.index < 0 ? 'idle' : s.playing ? 'playing' : 'ended',
    currentTime: s.position,
    duration: s.duration,
    isPlaying: s.playing,
    queueIndex: s.index,
    queueLength: s.queue.length,
    currentId: items[s.index]?.id,
    shuffle: s.shuffle,
    repeatMode: s.repeat,
    sleepTimerEndsAtMs: null,
    sleepTimerEndOfTrack: false,
    volume: s.volume,
  }
  nowPlaying.stateAt = performance.now()
  nowPlaying.from = ''
}
watch(
  () => playingOn.state,
  (s) => {
    if (s && playingOn.device) applyRemote(s)
    // Back to this device: its own player again.
    else if (!playingOn.device) syncNowPlaying().catch(() => {})
  },
)

/** A control: to the other device while playing there, else this device's player. */
function control(command: RemoteCommand, here: () => Promise<PlayerState>): Promise<unknown> {
  if (!playingOn.device) return shown(here())
  return remote.command(command).catch((e) => toast(`${playingOn.device?.name ?? 'The other device'}: ${e}`, MonitorSpeaker))
}

/** The other device's song at this place of its queue's list: its path on the share (null: a song only there). */
const pathThere = (index: number) => playingOn.state?.queue[index]?.path ?? null

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
  // Continued from another device whose song isn't here: that song, until this one plays.
  const stand = standIn.value
  if (stand && item && item.id === stand.waiting && !nowPlaying.state?.isPlaying) {
    return { id: -1, title: stand.title ?? 'A song', artist: stand.artist ?? '', duration: 0, liked: false, localOnly: false, later: `Only on ${stand.device}` } as Song
  }
  return item ? songOf(item) : null
})

// ---- Continue: a song this device doesn't have (sync/handoff.rs: StandIn)

/** Continued from another device whose song isn't here: what it plays (the player waits on the next song here). */
export const standIn = shallowRef<StandIn | null>(null)
let followingStandIn = false
/** Once (the app's frame): follows the stand-in; it goes once this device plays, or plays something else. */
export async function followStandIn() {
  if (followingStandIn) return
  followingStandIn = true
  await playbackSync.onStandIn((s) => (standIn.value = s)).catch(() => {})
  standIn.value = await playbackSync.standIn().catch(() => null)
  watch(
    () => [nowPlaying.state?.isPlaying, nowPlaying.state?.currentId] as const,
    ([playing, id]) => {
      const stand = standIn.value
      if (stand && (playing || (id != null && id !== stand.waiting))) {
        standIn.value = null
        playbackSync.standInDone().catch(() => {})
      }
    },
  )
}
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
export const togglePlay = () =>
  nowPlaying.state?.isPlaying ? control({ cmd: 'pause' }, () => player.pause()) : control({ cmd: 'play' }, () => player.play())
export const next = () => control({ cmd: 'next' }, () => player.next())
export const previous = () => control({ cmd: 'previous' }, () => player.previous())
export const seek = (seconds: number) => control({ cmd: 'seek', position: seconds }, () => player.seekTo(seconds))
export const setVolume = (volume: number) => {
  volume = snapVolume(volume)
  return control({ cmd: 'volume', volume }, () => player.setVolume(volume))
}

// ---- The volume's steps

/**
 * Playing on a phone: the steps its media volume moves in (the slider's, the wheel's, the keys'). "Volume steps on a
 * phone" levels a step; a phone with 30 levels or fewer steps through each (they're already like its buttons). 0: smooth.
 */
export const volumeSteps = computed(() => {
  const levels = (playingOn.device && playingOn.state?.volumeLevels) || 0
  return levels > 30 ? Math.max(1, Math.round(levels / Math.max(1, playingOn.phoneStep))) : levels
})
/** How far the volume keys and the wheel move it: a phone's step, else 5%. */
export const volumeStep = () => (volumeSteps.value ? 1 / volumeSteps.value : 0.05)
/** A volume (0 to 1) on the nearest step, with steps. */
export const snapVolume = (volume: number) => {
  const clamped = Math.min(1, Math.max(0, volume))
  return volumeSteps.value ? Math.round(clamped * volumeSteps.value) / volumeSteps.value : clamped
}
/** The volume as shown beside its slider: the step ("8/15") with steps, else the percentage. */
export const volumeLabel = (volume: number) =>
  volumeSteps.value ? `${Math.round(volume * volumeSteps.value)}/${volumeSteps.value}` : String(Math.round(volume * 100))
export const toggleShuffle = () => setShuffle(!nowPlaying.state?.shuffle)
export const setShuffle = (on: boolean) => control({ cmd: 'shuffle', on }, () => player.setShuffle(on))
const REPEAT_NEXT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' }
export const cycleRepeat = () => {
  const mode = REPEAT_NEXT[nowPlaying.state?.repeatMode ?? 'off']
  return control({ cmd: 'repeat', mode }, () => player.setRepeatMode(mode))
}
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
  const q = nowPlaying.queue
  const at = upNext.value.findIndex((u) => u.index === index)
  if (!q || at < 0) return
  // The song first in Up next, then on to it: the queue stays as it was (with shuffle too), nothing skipped over.
  const order = q.playOrder.length ? q.playOrder : q.items.map((_, i) => i)
  const start = order.indexOf(q.currentIndex) + 1
  const moved = [...order]
  moved.splice(start, 0, ...moved.splice(start + at, 1))
  // Shown at once as it ends up (not the steps): the tapped song playing, the songs under it moved up.
  if (nowPlaying.state?.shuffle) nowPlaying.queue = { ...q, playOrder: moved, currentIndex: index }
  else {
    const items = moved.map((i) => q.items[i])
    nowPlaying.queue = { ...q, items, currentIndex: items.indexOf(q.items[index]), playOrder: items.map((_, i) => i) }
  }
  if (nowPlaying.state) {
    nowPlaying.state = { ...nowPlaying.state, currentTime: 0, currentId: q.items[index].id, queueIndex: nowPlaying.queue!.currentIndex }
    nowPlaying.stateAt = performance.now()
  }
  // On another device: one step there (its next state is the result).
  if (playingOn.device) return control({ cmd: 'jump', index, path: pathThere(index) }, () => player.getState())
  // Here: the two steps, the queue read once at the end.
  await holdingQueue(async () => {
    if (at > 0) await player.moveInPlayOrder(start + at, start)
    await player.next()
    if (!(await player.getState()).isPlaying) await player.play()
  })
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
  // Playing on another device: moved there (its next state shows it).
  if (playingOn.device) return void control({ cmd: 'move', from: start + from, to: start + to, path: pathThere(order[start + from]) }, () => player.getState())
  await player.moveInPlayOrder(start + from, start + to)
  await refreshQueue()
}
export async function removeFromUpNext(at: number) {
  const { index } = upNext.value[at]
  takeOut([index])
  if (playingOn.device) return removeThere([index])
  await player.removeFromQueue(index)
  await refreshQueue()
}
/** Takes these queue items out (by their place in the queue's list), shown at once and read back once at the end. */
export async function takeOutOfQueue(indexes: number[]) {
  if (!indexes.length) return
  if (playingOn.device) return removeThere(indexes, true)
  takeOut(indexes)
  await holdingQueue(async () => {
    for (const index of [...indexes].sort((a, b) => b - a)) await player.removeFromQueue(index)
  })
}
export async function clearUpNext() {
  const indexes = upNext.value.map((u) => u.index)
  if (playingOn.device) return removeThere(indexes, true)
  takeOut(indexes)
  // From the end, so the indexes before stay right.
  await holdingQueue(async () => {
    for (const index of indexes.sort((a, b) => b - a)) await player.removeFromQueue(index)
  })
}
/** Playing on another device: songs out of its queue (by place, and which songs they are, so a changed queue isn't
 * edited in the wrong place). `show`: taken out here at once too. */
async function removeThere(indexes: number[], show = false) {
  const places = indexes.map((index) => ({ index, path: pathThere(index) }))
  if (show) takeOut(indexes)
  await control({ cmd: 'remove', places }, () => player.getState())
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
