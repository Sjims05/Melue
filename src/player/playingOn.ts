/**
 * Where this device's music plays (Connect device: player/remotePlayback.ts): this device, or another of yours, with
 * that one's state as it last said. Its own module, so the views' state (nowPlaying.ts) and Connect device can both
 * use it.
 */
import { reactive } from 'vue'
import type { RemoteState } from '@/library/remote'

export const playingOn = reactive({
  /** The other device played on (null: this one). */
  device: null as { id: string; name: string } | null,
  /** Its state, as it last said. */
  state: null as RemoteState | null,
  /** Connecting to this device (its id), for the menu to show it. */
  connecting: null as string | null,
  /** The other way round: the devices playing on this one now (their names). */
  playedFrom: [] as string[],
})

/** The device at the other end of the connection, by name (null: no connection). */
export const connectedTo = (): string | null => playingOn.device?.name ?? (playingOn.playedFrom.length ? playingOn.playedFrom.join(' and ') : null)

/** The connection in words: "Playing on <device>", "Played from <device>", or null. */
export function connectionText(): string | null {
  if (playingOn.device) return `Playing on ${playingOn.device.name}`
  return playingOn.playedFrom.length ? `Played from ${playingOn.playedFrom.join(' and ')}` : null
}
