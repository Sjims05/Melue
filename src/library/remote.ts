/**
 * Remote playback (src-tauri remote/): your devices, which are online, and whether this one lets the others play
 * on it.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/** "Let other devices play here": On, Ask first (someone there says yes), Off. */
export type Allow = 'on' | 'ask' | 'off'

export interface RemoteSettings {
  allow: Allow
}

export interface Device {
  id: string
  name: string
  kind: 'computer' | 'phone'
  online: boolean
  /** When it was last there (this device's clock). */
  lastSeenMs: number
  allow: Allow
  /** Where its server can be reached. */
  addresses: string[]
  port: number | null
  /** This device. */
  this: boolean
}

/** A command to the device played on. */
export type RemoteCommand =
  | { cmd: 'play' | 'pause' | 'next' | 'previous' }
  | { cmd: 'seek'; position: number }
  | { cmd: 'volume'; volume: number }
  | { cmd: 'shuffle'; on: boolean }
  | { cmd: 'repeat'; mode: 'off' | 'all' | 'one' }
  | { cmd: 'skipTo'; index: number }
  | { cmd: 'load'; queue: string[]; index: number; position: number; playing: boolean }

/** The device played on: its state, as it last said (`position` when it said it: count on while `playing`). */
export interface RemoteState {
  playing: boolean
  position: number
  duration: number
  volume: number
  shuffle: boolean
  repeat: 'off' | 'all' | 'one'
  index: number
  queue: { path: string | null; title: string | null; artist: string | null }[]
  /** The order the queue plays in (places in `queue`). */
  playOrder: number[]
}

export const remote = {
  settings: () => invoke<RemoteSettings>('remote_settings'),
  setSettings: (settings: RemoteSettings) => invoke<RemoteSettings>('remote_set_settings', { settings }),
  /** Your devices: this one first, then the online ones. */
  devices: () => invoke<Device[]>('remote_devices'),
  /** Reaches a device directly (the dev page's test): the address that answered, how long it took, what it said. */
  /** Connects to one of your devices, to play on it (one at a time). */
  connect: (id: string) => invoke<{ id: string; name: string; address: string }>('remote_connect', { id }),
  command: (command: RemoteCommand) => invoke<void>('remote_command', { command }),
  /** What plays here, to the device connected to. */
  sendPlaying: () => invoke<void>('remote_send_playing'),
  disconnect: () => invoke<void>('remote_disconnect'),
  /** The other device's songs (their paths on the share): their ids here, or null. */
  songIds: (paths: (string | null)[]) => invoke<(number | null)[]>('remote_song_ids', { paths }),
  /** Songs from here (their ids) to play on the device connected to, from song `index`. */
  load: (ids: number[], index: number, playing = true) => invoke<void>('remote_load', { ids, index, playing }),
  /** Connect device: what plays here plays there, and pauses here. */
  playOn: (id: string) => invoke<{ id: string; name: string; address: string }>('remote_play_on', { id }),
  /** "This device": what plays there plays here, and pauses there. */
  bringBack: () => invoke<void>('remote_bring_back'),
  /** Playing on another device now: which, and its state as it last said. */
  status: () => invoke<[{ id: string; name: string; address: string }, RemoteState | null] | null>('remote_status'),
  /** The devices playing on this one now (their names), and when that changes. */
  playedFrom: () => invoke<string[]>('remote_controlled_by'),
  onPlayedFrom: (listener: (names: string[]) => void) => listen<string[]>('remote-controlled', (e) => listener(e.payload)),
  /** Disconnects the devices playing on this one: what plays here goes on. */
  endPlayedFrom: () => invoke<void>('remote_end_control'),
  /** "Ask first": the answer to another device asking to play here. */
  answer: (id: number, yes: boolean) => invoke<void>('remote_answer', { id, yes }),
  /** Another device asks to play here (this one asks first). */
  onAsk: (listener: (asking: { id: number; from: string; seconds: number }) => void) =>
    listen<{ id: number; from: string; seconds: number }>('remote-ask', (e) => listener(e.payload)),
  onState: (listener: (state: RemoteState) => void) => listen<RemoteState>('remote-state', (e) => listener(e.payload)),
  onError: (listener: (error: string) => void) => listen<string>('remote-error', (e) => listener(e.payload)),
  /** The connection ended (`why`; null when this device closed it). */
  onClosed: (listener: (why: string | null) => void) => listen<string | null>('remote-closed', (e) => listener(e.payload)),
  reach: (id: string) => invoke<{ address: string; ms: number; hello: { deviceId: string; name: string; version: string; allow: Allow } }>('remote_reach', { id }),
}
