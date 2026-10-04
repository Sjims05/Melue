/**
 * Playing from the library: starting a list, and what the player reports back. `startPlayback()` runs
 * once at app start: it sets the player's extra buttons (repeat, shuffle, like) and counts playback events
 * and like presses, including those from while the app wasn't running (Android Auto); the player keeps
 * them until they're handed over.
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
import { player } from '@/platform'
import { library, type PlaybackOutcome, type Source } from '.'

const LIKE_BUTTON = 'like'

/** Plays a list from its start, or from the song `startAt` (a song id in it). */
export async function playSource(source: Source, options: { startAt?: number } = {}): Promise<void> {
  const items = await library.queueItems(source)
  if (items.length === 0) throw new Error('nothing to play in that list')
  const startIndex = Math.max(0, items.findIndex((item) => item.id === options.startAt))
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

/** Once at app start: the player's buttons, and counting what it plays. */
export async function startPlayback(): Promise<void> {
  if (started) return
  started = true
  await player.initialize()
  await setControls({
    buttons: [{ action: 'repeat' }, { action: 'shuffle' }, { action: 'custom', id: LIKE_BUTTON, label: 'Like', icon: 'heart', toggle: true }],
  })
  // The like button shows the library's likes (it remembers them on the device, for Android Auto).
  const liked = await library.likedIds()
  if (liked.length > 0) await setControlActive(LIKE_BUTTON, liked, true)
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
