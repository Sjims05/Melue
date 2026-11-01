/**
 * The first-start guide (FirstStartGuide.vue): shown once on each device, when Melue starts without a share set up
 * (a fresh start; a device that syncs already has been through it, or doesn't need it). About shows it again, and
 * Melue Dev's Dev page too.
 */
import { invoke } from '@tauri-apps/api/core'
import { reactive } from 'vue'
import { library } from '@/library'

/** Seen (finished or skipped) on this device. */
const DONE = 'firstStart.done'

/** `pretend`: as on a new device (Melue Dev): the share and the folders as on a fresh start, and nothing saved. */
export const firstStart = reactive({ open: false, pretend: false })

/** At start: the guide, if it's this device's first start. */
export async function showFirstStartOnce() {
  if ((await library.setting(DONE).catch(() => '1')) === '1') return
  const shared = await invoke<boolean>('library_sync_available').catch(() => true)
  if (shared) return void (await library.setSetting(DONE, '1').catch(() => {}))
  firstStart.pretend = false
  firstStart.open = true
}

/** Again (About, the Dev page); `pretend`: as on a new device. */
export function showFirstStart(pretend = false) {
  firstStart.pretend = pretend
  firstStart.open = true
}

/** Finished or skipped: not shown again by itself. */
export async function closeFirstStart() {
  firstStart.open = false
  if (firstStart.pretend) return
  await library.setSetting(DONE, '1').catch(() => {})
}
