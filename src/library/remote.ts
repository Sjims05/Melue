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
  onState: (listener: (state: RemoteState) => void) => listen<RemoteState>('remote-state', (e) => listener(e.payload)),
  onError: (listener: (error: string) => void) => listen<string>('remote-error', (e) => listener(e.payload)),
  /** The connection ended (`why`; null when this device closed it). */
  onClosed: (listener: (why: string | null) => void) => listen<string | null>('remote-closed', (e) => listener(e.payload)),
  reach: (id: string) => invoke<{ address: string; ms: number; hello: { deviceId: string; name: string; version: string; allow: Allow } }>('remote_reach', { id }),
}
