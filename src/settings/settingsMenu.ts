/**
 * The settings page's right-click extras (components/settings/settingMenu.ts): the defaults and help texts from the
 * registry, Reset to default (with Undo, shared like any change), copying a setting's link
 * (melue://settings/sync.atStart), and "This device only".
 *
 * This device only: kept in Melue's settings on this device (the list of ids). Settings don't sync across devices
 * yet; when they do, these stay this device's own.
 */
import { RotateCcw } from '@lucide/vue'
import { reactive } from 'vue'
import type { SettingMenu } from '@/components/settings'
import { library } from '@/library'
import { SETTINGS } from './registry'
import { toast } from '@/lib/toast'
import { DEVICE_ONLY, readSetting, rejoinShared, settingsTouched, writeSetting } from './sharedSettings'

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
    async reset(rows) {
      // What each was, for Undo: a reset's own undo, or the value read now.
      const before = await Promise.all(rows.map((row) => (row.setting ? readSetting(row.setting) : undefined)))
      const undos = rows.map((row) => row.reset())
      settingsTouched()
      toast(rows.length === 1 ? 'Reset to default' : `${rows.length} settings reset to default`, RotateCcw, {
        label: 'Undo',
        run: async () => {
          for (const [i, row] of rows.entries()) {
            const undo = undos[i]
            if (typeof undo === 'function') await undo()
            else if (row.setting && before[i] !== undefined) await writeSetting(row.setting, before[i])
          }
          settingsTouched()
        },
      })
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
  // Once: the class goes when the flash ends (left on, coming back to Settings would play it again, on every row it
  // was ever on).
  document.querySelectorAll('.settings-flash').forEach((el) => el.classList.remove('settings-flash'))
  requestAnimationFrame(() => {
    row.classList.add('settings-flash')
    const done = (e: AnimationEvent) => {
      if (e.animationName !== 'settings-flash') return
      row.classList.remove('settings-flash')
      row.removeEventListener('animationend', done)
      row.removeEventListener('animationcancel', done)
    }
    // Ended, or cut short (Settings left during it).
    row.addEventListener('animationend', done)
    row.addEventListener('animationcancel', done)
  })
}
