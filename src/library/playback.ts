/**
 * Playing from the library: starting a list, and what the player reports back. `startPlayback()` runs
 * once at app start: it sets the player's extra buttons (repeat, shuffle, like) and its list of recently
 * played lists, and counts playback events and like presses, including those from while the app wasn't
 * running (Android Auto); the player keeps them until they're handed over. The Android Auto library itself
 * is sent from Rust (auto.rs).
 *
 * Keeping the queue in step with library changes happens in Rust (follow.rs), so it works while the UI
 * is asleep.
 */
import {
  onControlPressed,
  onPlaybackEvent,
  setControlActive,
  setControls,
  type NativeAudioPlaybackEvent,
} from 'tauri-plugin-native-audio-api'
import { invoke } from '@tauri-apps/api/core'
import { isMobile, player, trackedLists } from '@/platform'
import { library, type PlaybackOutcome, type PlayerSettings, type Source } from '.'

const LIKE_BUTTON = 'like'
/** The lists played (albums, playlists, artists), most recent first: Android Auto's "Recently played" tab. */
export const RECENT_LISTS = 'recentLists'

/**
 * Plays a list from its start, or from the song `startAt` (a song id in it). With shuffle on and no song picked, it
 * starts on a random one of its songs (the rest shuffled after it), not always its first.
 */
export async function playSource(source: Source, options: { startAt?: number } = {}): Promise<void> {
  const items = await library.queueItems(source)
  if (items.length === 0) throw new Error('nothing to play in that list')
  const picked = items.findIndex((item) => item.id === options.startAt)
  const shuffled = picked < 0 && (await player.getState().catch(() => null))?.shuffle
  const startIndex = picked >= 0 ? picked : shuffled ? Math.floor(Math.random() * items.length) : 0
  await player.setQueue(items, { sourceId: source, startIndex })
  await player.play()
}

/** Likes or unlikes a song, and shows it on the player's like button. */
export async function setLiked(id: number, liked: boolean): Promise<void> {
  await library.setLikedOnly(id, liked)
  await setControlActive(LIKE_BUTTON, [id], liked).catch(() => {})
}

type CountedListener = (event: NativeAudioPlaybackEvent, outcome: PlaybackOutcome | null) => void
const countedListeners: CountedListener[] = []

/**
 * Calls `listener` after each playback event is counted, with what it counted as. (Only startPlayback
 * takes the events from the player, so none are lost to another listener.)
 */
export function onPlaybackCounted(listener: CountedListener): () => void {
  countedListeners.push(listener)
  return () => countedListeners.splice(countedListeners.indexOf(listener), 1)
}

let started = false

/** Hands the player settings Melue keeps to the player (all of them, or a change). */
export async function applyPlayerSettings(settings: PlayerSettings, change: Partial<PlayerSettings> = settings) {
  const { skipSeconds, ...options } = change
  if (Object.keys(options).length > 0) await player.setOptions(options)
  if (skipSeconds !== undefined) await player.setSkipInterval(settings.skipSeconds)
}

/** Once at app start: the player's buttons, and counting what it plays. */
export async function startPlayback(): Promise<void> {
  if (started) return
  started = true
  await player.initialize()
  // The player settings (it saves them too; this keeps it in line with Melue's, e.g. after a reinstall).
  const settings = await library.playerSettings().catch((e) => void console.error('melue: player settings', e))
  if (settings) await applyPlayerSettings(settings).catch((e) => console.error('melue: player settings', e))
  // The last queue, as "When Melue starts" says. The computer's player brings it back by itself; the phone's only when
  // its background service starts (not when the app opens), so it's asked for here. Both leave a queue that's already
  // loaded alone (playing in the background, in the car).
  if (settings && settings.resumeLastQueue !== 'off') {
    const restored = await player.restoreLastQueue().catch(() => null)
    if (restored && isMobile && settings.resumeLastQueue === 'play' && !restored.isPlaying) await player.play().catch(() => {})
  }
  // The player keeps this itself, also for lists played in the car with the app closed (auto.rs uses it).
  await trackedLists.configure([{ id: RECENT_LISTS, track: 'folder', limit: 20 }])
  await setControls({
    buttons: [{ action: 'repeat' }, { action: 'shuffle' }, { action: 'custom', id: LIKE_BUTTON, label: 'Like', icon: 'heart', toggle: true }],
  })
  // The like button shows the library's likes (it remembers them on the device, for Android Auto).
  const liked = await library.likedIds()
  if (liked.length > 0) await setControlActive(LIKE_BUTTON, liked, true)
  // Likes from other devices (and anything else that changes them): the button follows.
  await library.onChange(async (change) => {
    if (change.reason !== 'likes' || change.tracks.length === 0) return
    const now = new Set(await library.likedIds())
    const liked = change.tracks.filter((id) => now.has(id))
    const unliked = change.tracks.filter((id) => !now.has(id))
    if (liked.length > 0) await setControlActive(LIKE_BUTTON, liked, true).catch(() => {})
    if (unliked.length > 0) await setControlActive(LIKE_BUTTON, unliked, false).catch(() => {})
  })
  await onControlPressed(async (press) => {
    if (press.buttonId === LIKE_BUTTON && press.itemId != null && press.active != null) {
      await library.setLikedOnly(press.itemId, press.active)
    }
  })
  await onPlaybackEvent(async (event: NativeAudioPlaybackEvent) => {
    const outcome = await invoke<PlaybackOutcome | null>('library_record_playback', { event })
    for (const listener of countedListeners) listener(event, outcome)
  })
}
