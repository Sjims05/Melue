/**
 * The player the app talks to, whatever plays the audio underneath:
 * the native-audio plugin on every platform (NativePlayer).
 *
 * The queue lives in the player itself, so it keeps going while the UI is suspended.
 * Shapes match the native-audio plugin, so NativePlayer can pass calls straight through.
 */

export type RepeatMode = 'off' | 'all' | 'one'

export type PlayerStatus = 'idle' | 'loading' | 'playing' | 'ended' | 'error'

export interface PlayerTrack {
  /** Stable id, 1 or higher (the plugin ignores ids <= 0 for progress tracking). */
  id: number
  /** Local file path. */
  src: string
  title?: string
  artist?: string
  album?: string
  artworkUrl?: string
}

export interface PlayerState {
  status: PlayerStatus
  /** Seconds. */
  currentTime: number
  /** Seconds. */
  duration: number
  isPlaying: boolean
  queueIndex: number
  queueLength: number
  currentId?: number
  shuffle: boolean
  repeatMode: RepeatMode
  error?: string
  /** The player's own volume, 0 to 1, on top of the system volume. */
  volume: number
}

export interface PlayerQueue {
  /** In the order they were added. */
  items: PlayerTrack[]
  /** Index into `items` of the current track. */
  currentIndex: number
  /** Indexes into `items` in the order they play (differs from `items` while shuffled). */
  playOrder: number[]
}

export interface OutputDevice {
  id: string
  name: string
  isDefault: boolean
}

export interface OutputDevices {
  /** Empty where the system picks the output itself (Android). */
  devices: OutputDevice[]
  /** The chosen device's id, or null to follow the system default. */
  selected: string | null
  /** The device playing now. */
  active: string | null
}

export interface SetQueueOptions {
  startIndex?: number
  /** Seconds into the first track. */
  startPosition?: number
  /** The playlist / album the queue was started from (used for recently played). */
  sourceId?: string
}

export interface Player {
  /** Call once before anything else. */
  initialize(): Promise<PlayerState>
  getState(): Promise<PlayerState>
  /** Calls `listener` on every state change. Resolves to a function that stops listening. */
  onState(listener: (state: PlayerState) => void): Promise<() => void>

  play(): Promise<PlayerState>
  pause(): Promise<PlayerState>
  /** Seconds. */
  seekTo(position: number): Promise<PlayerState>
  next(): Promise<PlayerState>
  previous(): Promise<PlayerState>
  setShuffle(enabled: boolean): Promise<PlayerState>
  setRepeatMode(mode: RepeatMode): Promise<PlayerState>

  setQueue(items: PlayerTrack[], options?: SetQueueOptions): Promise<PlayerState>
  getQueue(): Promise<PlayerQueue>
  /** Index into the queue's `items`. */
  skipTo(index: number): Promise<PlayerState>
  addToQueue(items: PlayerTrack[], options?: { playNext?: boolean }): Promise<PlayerState>
  /** Index into the queue's `items`. */
  removeFromQueue(index: number): Promise<PlayerState>
  moveInQueue(from: number, to: number): Promise<PlayerState>
  /** Loads the queue that was playing last time (paused). Null when there is none. */
  restoreLastQueue(): Promise<PlayerState | null>

  /** 0 to 1. */
  setVolume(volume: number): Promise<PlayerState>
  getOutputDevices(): Promise<OutputDevices>
  /** A device id from getOutputDevices, or null for the system default. Not saved by the player. */
  setOutputDevice(id: string | null): Promise<OutputDevices>
}
