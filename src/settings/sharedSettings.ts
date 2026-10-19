/**
 * Settings across devices (Melue's side: sync/settings_sync.rs). Every setting in the registry is shared, except the
 * ones that are this device's own (marked perDevice) and the ones set to "This device only" (the right-click menu).
 * The share's layout is left out: it lives on the share already.
 *
 * The settings live in different places (this app's storage, Melue's settings, the player's), so each has a way to be
 * read and set here (ADAPTERS). What changed here is told to Melue (`check`): at start, a few seconds after a change on
 * the settings page or to the app's own settings, and Melue sends it on with its next library data sync; settings
 * another device changed come back as an event, and are set here.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { watch } from 'vue'
import { library, type PlayerSettings, type QueueFollow } from '@/library'
import { applyPlayerSettings } from '@/library/playback'
import { playbackSync, syncing, sync, type PlaybackSettings, type SyncSettings } from '@/library/sync'
import { appearance } from '@/lib/appearance'
import { listPlay } from '@/lib/listPlay'
import { themeMode, themeTimes, type ThemeMode } from '@/lib/theme'
import { playerSettings } from './playerSettings'
import { SETTINGS } from './registry'
import { syncSettings } from './syncSettings'

/** Melue's setting with the ids set to "This device only" (the right-click menu, settingsMenu.ts). */
export const DEVICE_ONLY = 'device_only_settings'

type Adapter = { read: () => unknown | Promise<unknown>; write: (value: unknown) => void | Promise<void> }

/** A setting that's a field of a group saved together (read the group, change the field, save the group). */
function fieldOf<T extends object>(get: () => Promise<T>, save: (all: T) => Promise<unknown>, after?: (all: T) => void) {
  return (key: keyof T): Adapter => ({
    read: async () => (await get())[key],
    write: async (value) => {
      const all = { ...(await get()), [key]: value }
      await save(all)
      after?.(all)
    },
  })
}

const syncField = fieldOf<SyncSettings>(syncing.settings, syncing.setSettings, (all) => syncSettings.value && (syncSettings.value = all))
const playbackField = fieldOf<PlaybackSettings>(playbackSync.settings, playbackSync.setSettings)
const followField = fieldOf<QueueFollow>(library.queueFollow, library.setQueueFollow)
const playerField = (key: keyof PlayerSettings): Adapter => ({
  read: async () => (await library.playerSettings())[key],
  write: async (value) => {
    const all = { ...(await library.playerSettings()), [key]: value }
    await library.setPlayerSettings(all)
    await applyPlayerSettings(all, { [key]: value })
    if (playerSettings.value) playerSettings.value = all
  },
})
const own = <T extends object>(store: T, key: keyof T): Adapter => ({
  read: () => store[key],
  write: (value) => void ((store as Record<keyof T, unknown>)[key] = value),
})
const topSize: Adapter = {
  read: async () => (await library.autoPlaylists()).find((p) => p.kind === 'top')?.size ?? 50,
  write: async (size) => {
    const tops = (await library.autoPlaylists()).filter((p) => p.kind === 'top')
    await Promise.all(tops.map((p) => library.setAutoPlaylist(p.source, { size: size as number })))
  },
}

const ADAPTERS: Record<string, Adapter> = {
  'playback.mode': playbackField('mode'),
  'playback.continueWithinMinutes': playbackField('continueWithinMinutes'),
  'sync.atStart': syncField('atStart'),
  'sync.onReturnAfterMinutes': syncField('onReturnAfterMinutes'),
  'sync.everyMinutes': syncField('everyMinutes'),
  'sync.removeGone': syncField('removeGone'),
  'sync.askBeforeRemoving': syncField('askBeforeRemoving'),
  'sync.moveNewSongs': syncField('moveNewSongs'),
  'queue.enabled': followField('enabled'),
  'queue.removedCurrent': followField('removedCurrent'),
  'queue.removedItems': followField('removedItems'),
  'queue.newItems': followField('newItems'),
  'queue.order': followField('order'),
  'queue.playFromStart': own(listPlay, 'playFromStart'),
  'queue.songPlays': own(listPlay, 'songPlays'),
  'player.nextAtEnd': playerField('nextAtEnd'),
  'player.repeatAddedTracks': playerField('repeatAddedTracks'),
  'player.volumeCurve': playerField('volumeCurve'),
  'player.mediaControls': playerField('mediaControls'),
  'player.skipSeconds': playerField('skipSeconds'),
  'player.resumeLastQueue': playerField('resumeLastQueue'),
  'player.previousRestartsAfterSeconds': playerField('previousRestartsAfterSeconds'),
  'player.pausedKeepAliveMinutes': playerField('pausedKeepAliveMinutes'),
  'player.keepAliveWhileCarConnected': playerField('keepAliveWhileCarConnected'),
  'playlists.topSize': topSize,
  'display.theme': { read: () => themeMode.value, write: (mode) => void (themeMode.value = mode as ThemeMode) },
  'display.darkFrom': own(themeTimes, 'dark'),
  'display.lightFrom': own(themeTimes, 'light'),
  'display.upNextPrimary': own(appearance, 'upNextPrimary'),
  'display.homeCardsPrimary': own(appearance, 'homeCardsPrimary'),
}

/** The settings shared now: not this device's own, nor set to "This device only". */
async function sharedIds(): Promise<string[]> {
  const deviceOnly = new Set<string>(JSON.parse((await library.setting(DEVICE_ONLY).catch(() => null)) ?? '[]'))
  const perDevice = (id: string) => (SETTINGS as Record<string, { perDevice?: boolean }>)[id]?.perDevice
  return Object.keys(ADAPTERS).filter((id) => !perDevice(id) && !deviceOnly.has(id))
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)
// Being set from another device: not told back as a change here.
let applying = false

/** Tells Melue the shared settings that changed here since it last knew them. */
async function check() {
  if (!sync.available || applying) return
  const known = (await invoke<Record<string, unknown> | null>('shared_settings')) ?? {}
  const first = Object.keys(known).length === 0
  const changes: Record<string, unknown> = {}
  for (const id of await sharedIds()) {
    const value = await Promise.resolve(ADAPTERS[id].read()).catch(() => undefined)
    if (value !== undefined && !(id in known && same(known[id], value))) changes[id] = value
  }
  if (Object.keys(changes).length) await invoke('record_shared_settings', { changes, first })
}

async function apply(settings: Record<string, unknown>) {
  const ids = new Set(await sharedIds())
  applying = true
  try {
    for (const [id, value] of Object.entries(settings)) {
      if (ids.has(id)) await Promise.resolve(ADAPTERS[id].write(value)).catch((e) => console.error(`melue: setting ${id}`, e))
    }
  } finally {
    applying = false
  }
}

/**
 * A setting no longer "This device only": it takes the latest shared value (what the other devices have), so all
 * devices start from the same place again. Nothing to do without a share, or when it was never shared.
 */
export async function rejoinShared(id: string) {
  if (!sync.available || !ADAPTERS[id]) return
  const known = (await invoke<Record<string, unknown> | null>('shared_settings')) ?? {}
  if (!(id in known)) return
  applying = true
  try {
    await ADAPTERS[id].write(known[id])
  } finally {
    applying = false
  }
}

let waiting: ReturnType<typeof setTimeout> | undefined
/** A setting may have changed (the settings page was used): checked a few seconds from now. */
export function settingsTouched() {
  clearTimeout(waiting)
  waiting = setTimeout(() => check().catch((e) => console.error('melue: shared settings', e)), 3000)
}

let started = false
/** Once (the app's frame): shares the settings across devices while a share is set up. */
export function followSharedSettings() {
  if (started) return
  started = true
  listen<Record<string, unknown>>('shared-settings', (e) => apply(e.payload)).catch(() => {})
  watch(() => sync.available, (available) => available && settingsTouched(), { immediate: true })
  // The app's own settings (kept here, changed anywhere in the app): checked after they change.
  watch([() => ({ ...listPlay }), () => ({ ...appearance }), themeMode, () => ({ ...themeTimes })], () => !applying && settingsTouched())
}
