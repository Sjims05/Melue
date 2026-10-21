/**
 * The player's settings, shared by the settings components that show them (audio, the end of the queue): read once,
 * and saved as they change. Melue keeps them (the player can't read them back); each change also goes to the player
 * (which saves it too). A failed save puts the old value back.
 */
import { reactive } from 'vue'
import { resetTo } from '@/components/settings/settingMenu'
import { library, type PlayerSettings } from '@/library'
import { applyPlayerSettings } from '@/library/playback'
import { PLAYER_DEFAULTS } from './defaults'

export const playerSettings = reactive({
  value: null as PlayerSettings | null,
  /** Why the last read or save didn't work (cleared by the next save). */
  error: '',
})

let loading: Promise<void> | null = null

export function loadPlayerSettings(): Promise<void> {
  loading ??= library
    .playerSettings()
    .then((settings) => {
      playerSettings.value = settings
    })
    .catch((e) => {
      playerSettings.error = `Can’t read the player settings: ${e}`
      loading = null
    })
  return loading
}

export async function savePlayerSettings(change: Partial<PlayerSettings>): Promise<void> {
  const before = playerSettings.value
  if (!before) return
  playerSettings.value = { ...before, ...change }
  playerSettings.error = ''
  try {
    playerSettings.value = await library.setPlayerSettings(playerSettings.value)
    await applyPlayerSettings(playerSettings.value, change)
  } catch (e) {
    playerSettings.value = before
    playerSettings.error = `Not saved: ${e}`
  }
}

/** A row's "Reset to default" for one setting: `<SettingsRow v-bind="resetPlayer('volumeCurve')">`. */
export function resetPlayer<K extends keyof PlayerSettings>(key: K) {
  return {
    setting: `player.${key}`,
    reset: resetTo(() => playerSettings.value?.[key] ?? PLAYER_DEFAULTS[key], (value) => savePlayerSettings({ [key]: value }), PLAYER_DEFAULTS[key]),
    isDefault: playerSettings.value?.[key] === PLAYER_DEFAULTS[key],
  }
}
