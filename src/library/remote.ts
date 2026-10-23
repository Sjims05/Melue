/**
 * Remote playback (src-tauri remote/): your devices, which are online, and whether this one lets the others play
 * on it.
 */
import { invoke } from '@tauri-apps/api/core'

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

export const remote = {
  settings: () => invoke<RemoteSettings>('remote_settings'),
  setSettings: (settings: RemoteSettings) => invoke<RemoteSettings>('remote_set_settings', { settings }),
  /** Your devices: this one first, then the online ones. */
  devices: () => invoke<Device[]>('remote_devices'),
  /** Reaches a device directly (the dev page's test): the address that answered, how long it took, what it said. */
  reach: (id: string) => invoke<{ address: string; ms: number; hello: { deviceId: string; name: string; version: string; allow: Allow } }>('remote_reach', { id }),
}
