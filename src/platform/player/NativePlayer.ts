import * as audio from 'tauri-plugin-native-audio-api'
import type { Player, PlayerOptions, PlayerQueue, PlayerTrack, RepeatMode, SetQueueOptions, UpdateQueueOptions } from './Player'

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
  dispose = () => audio.dispose()
  clear = () => audio.clear()
  onState = async (listener: Parameters<Player['onState']>[0]) => {
    const stop = await audio.addStateListener(listener)
    // The plugin sends state when it changes; a paused, restored queue doesn't, so start with the current one.
    listener(await audio.getState())
    return stop
  }

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
  updateQueue = (items: PlayerTrack[], options?: UpdateQueueOptions) => audio.updateQueue(items, options)
  removeFromQueue = (index: number) => audio.removeFromQueue(index)
  moveInQueue = (from: number, to: number) => audio.moveInQueue(from, to)
  moveInPlayOrder = (from: number, to: number) => audio.moveInPlayOrder(from, to)
  restoreLastQueue = () => audio.restoreLastQueue()

  setSleepTimer = (timer: Parameters<Player['setSleepTimer']>[0]) => audio.setSleepTimer(timer)
  cancelSleepTimer = () => audio.cancelSleepTimer()

  setVolume = (volume: number) => audio.setVolume(volume)
  getOutputDevices = () => audio.getOutputDevices()
  getOffloadSupport = () => audio.getOffloadSupport()
  setOutputDevice = (id: string | null) => audio.setOutputDevice(id)
  setOptions = (options: PlayerOptions) => audio.setOptions(options)
  setSkipInterval = (seconds: number) => audio.setSkipInterval(seconds)
}
