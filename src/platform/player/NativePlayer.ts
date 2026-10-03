import * as audio from 'tauri-plugin-native-audio-api'
import type { Player, PlayerQueue, PlayerTrack, RepeatMode, SetQueueOptions } from './Player'

/**
 * Every platform: everything goes to the native-audio plugin. On Android / iOS it plays in a native
 * media service (background playback, lock screen, notification, Android Auto); on desktop in its
 * Rust player.
 *
 * The plugin's other features (tracked lists, options, buttons, the car library) aren't part of Player
 * yet; for now the mobile code uses the plugin for those directly.
 */
export class NativePlayer implements Player {
  initialize = () => audio.initialize()
  getState = () => audio.getState()
  onState = (listener: Parameters<Player['onState']>[0]) => audio.addStateListener(listener)

  play = () => audio.play()
  pause = () => audio.pause()
  seekTo = (position: number) => audio.seekTo(position)
  next = () => audio.next()
  previous = () => audio.previous()
  setShuffle = (enabled: boolean) => audio.setShuffle(enabled)
  setRepeatMode = (mode: RepeatMode) => audio.setRepeatMode(mode)

  setQueue = (items: PlayerTrack[], options: SetQueueOptions = {}) => audio.setQueue({ items, ...options })
  // The plugin hands back the items it was given, and Player always gives them ids.
  getQueue = () => audio.getQueue() as Promise<PlayerQueue>
  skipTo = (index: number) => audio.skipTo(index)
  addToQueue = (items: PlayerTrack[], options?: { playNext?: boolean }) => audio.addToQueue(items, options)
  removeFromQueue = (index: number) => audio.removeFromQueue(index)
  moveInQueue = (from: number, to: number) => audio.moveInQueue(from, to)
  restoreLastQueue = () => audio.restoreLastQueue()
}
