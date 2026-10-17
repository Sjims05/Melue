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
  /** When a running sleep timer pauses playback (epoch ms), or null. */
  sleepTimerEndsAtMs: number | null
  /** A sleep timer that pauses at the end of the current track. */
  sleepTimerEndOfTrack: boolean
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
  /** The list the queue was started from (setQueue's `sourceId`: 'album:3', 'liked'…), or null. */
  sourceId?: string | null
  /**
   * With shuffle and repeat all: the order the next pass plays in (indexes into `items`; it's shuffled again each time
   * the queue starts over), worked out ahead. Null otherwise: without shuffle the next pass is the list.
   */
  nextPassOrder?: number[] | null
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

/** The player's options (the plugin's setOptions). */
export interface PlayerOptions {
  volumeCurve?: 'linear' | 'quadratic' | 'cubic'
  mediaControls?: boolean
  resumeLastQueue?: 'off' | 'paused' | 'play'
  previousRestartsAfterSeconds?: number
  nextAtEnd?: 'nothing' | 'first'
  repeatAddedTracks?: boolean
  pausedKeepAliveMinutes?: number
  keepAliveWhileCarConnected?: boolean
}

export interface UpdateQueueOptions {
  /** The playlist / album the queue now plays; unchanged when left out. */
  sourceId?: string
  /** The playing track left the playlist: let it `finish` (default), or `skip` to the next now. */
  removedCurrent?: 'finish' | 'skip'
  /** Tracks no longer in the playlist: `remove` (default) or `keep` them in the queue. */
  removedItems?: 'remove' | 'keep'
  /** New tracks: at their place in the playlist (`inPlace`, default), at the `end` (shuffled there when shuffling), or `next`. */
  newItems?: 'inPlace' | 'end' | 'next'
  /** The list order: the `playlist`'s new order (default) or the `queue`'s current one. */
  order?: 'playlist' | 'queue'
}

export interface UpdateQueueResult {
  state: PlayerState
  /** Tracks new to the playlist. */
  added: number
  /** Tracks no longer in the playlist. */
  removed: number
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
  /** Stops playback and empties the queue (the player bar's ✕). The queue saved for restoreLastQueue stays. */
  dispose(): Promise<void>
  /** Calls `listener` with the current state, then on every change. Resolves to a function that stops listening. */
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
  /**
   * The playlist the queue plays changed (e.g. after a sync): `items` is its new content, matched by
   * id. What stays keeps its place, new tracks are added, missing ones removed (see the options);
   * tracks added with addToQueue stay.
   */
  updateQueue(items: PlayerTrack[], options?: UpdateQueueOptions): Promise<UpdateQueueResult>
  /** Index into the queue's `items`. */
  removeFromQueue(index: number): Promise<PlayerState>
  moveInQueue(from: number, to: number): Promise<PlayerState>
  /** Places in the play order (`playOrder`): what Up next shows; with shuffle on, only the play order changes. */
  moveInPlayOrder(from: number, to: number): Promise<PlayerState>
  /** Loads the queue that was playing last time (paused). Null when there is none. */
  restoreLastQueue(): Promise<PlayerState | null>

  /**
   * Pauses after `minutes`, fading out over the last `fadeOutSeconds` (default 10), or at the end
   * of the current track. Replaces a running timer.
   */
  setSleepTimer(timer: { minutes: number; fadeOutSeconds?: number } | { endOfTrack: true }): Promise<PlayerState>
  cancelSleepTimer(): Promise<PlayerState>

  /** 0 to 1. */
  setVolume(volume: number): Promise<PlayerState>
  getOutputDevices(): Promise<OutputDevices>
  /** A device id from getOutputDevices, or null for the system default. Saved by the player. */
  setOutputDevice(id: string | null): Promise<OutputDevices>
  /** The player's options (only those passed change). Saved by the player. */
  setOptions(options: PlayerOptions): Promise<void>
  /** What previous / next do on media keys, headphones and the notification: 0 skips tracks, above 0 seeks. */
  setSkipInterval(seconds: number): Promise<PlayerState>
}
