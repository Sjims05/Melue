/**
 * Melue's sync settings, shared by the settings components that show them (triggers, songs, the share's layout,
 * mobile data): read once, and saved as they change (a failed save puts the old value back).
 */
import { reactive } from 'vue'
import { syncing, type SyncSettings } from '@/library/sync'
import { SYNC_DEFAULTS } from './defaults'

export const syncSettings = reactive({
  value: null as SyncSettings | null,
  /** Why the last read or save didn't work (cleared by the next save). */
  error: '',
})

let loading: Promise<void> | null = null

export function loadSyncSettings(): Promise<void> {
  loading ??= syncing
    .settings()
    .then((settings) => {
      syncSettings.value = settings
    })
    .catch((e) => {
      syncSettings.error = `Can’t read the sync settings: ${e}`
      loading = null
    })
  return loading
}

export async function saveSyncSettings(change: Partial<SyncSettings>): Promise<void> {
  const before = syncSettings.value
  if (!before) return
  syncSettings.value = { ...before, ...change }
  syncSettings.error = ''
  try {
    syncSettings.value = await syncing.setSettings(syncSettings.value)
  } catch (e) {
    syncSettings.value = before
    syncSettings.error = `Not saved: ${e}`
  }
}

/** A row's "Reset to default" for one setting: `<SettingsRow v-bind="resetSync('atStart')">`. */
export function resetSync<K extends keyof SyncSettings>(key: K) {
  return {
    setting: `sync.${key}`,
    reset: () => saveSyncSettings({ [key]: SYNC_DEFAULTS[key] }),
    isDefault: syncSettings.value?.[key] === SYNC_DEFAULTS[key],
  }
}
