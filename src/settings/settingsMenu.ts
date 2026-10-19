/**
 * The settings page's right-click extras (components/settings/settingMenu.ts): the defaults and help texts from the
 * registry, copying a setting's link (melue://settings/sync.atStart), and "This device only".
 *
 * This device only: kept in Melue's settings on this device (the list of ids). Settings don't sync across devices
 * yet; when they do, these stay this device's own.
 */
import { reactive } from 'vue'
import type { SettingMenu } from '@/components/settings'
import { library } from '@/library'
import { SETTINGS } from './registry'
import { DEVICE_ONLY, rejoinShared } from './sharedSettings'

export const LINK = 'melue://settings/'

const deviceOnly = reactive(new Set<string>())
let loaded = false

async function load() {
  if (loaded) return
  loaded = true
  try {
    for (const id of JSON.parse((await library.setting(DEVICE_ONLY)) ?? '[]') as string[]) deviceOnly.add(id)
  } catch {
    // None marked yet.
  }
}

export function createSettingsMenu(): SettingMenu {
  load()
  const menu: SettingMenu = reactive({
    info: (id: string) => SETTINGS[id],
    isDeviceOnly: (id: string) => deviceOnly.has(id),
    setDeviceOnly(id: string, on: boolean) {
      if (on) deviceOnly.add(id)
      else {
        deviceOnly.delete(id)
        // Shared again: it takes the other devices' value, the same starting point everywhere.
        rejoinShared(id).catch((e) => console.error('melue: shared setting', e))
      }
      library.setSetting(DEVICE_ONLY, JSON.stringify([...deviceOnly])).catch((e) => console.error('melue: device-only settings', e))
    },
    copied: null as string | null,
    async copyLink(id: string) {
      await navigator.clipboard.writeText(`${LINK}${id}`)
      menu.copied = id
      setTimeout(() => menu.copied === id && (menu.copied = null), 1600)
    },
  })
  return menu
}

/** Scrolls to a setting's row and lights it up briefly (a link to it: melue://settings/<id>, or ?setting=<id>). */
export function goToSetting(id: string) {
  const row = document.querySelector<HTMLElement>(`[data-setting="${CSS.escape(id)}"]`)
  if (!row) return
  row.scrollIntoView({ behavior: 'smooth', block: 'center' })
  row.classList.remove('settings-flash')
  requestAnimationFrame(() => row.classList.add('settings-flash'))
}
