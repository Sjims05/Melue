/**
 * Whether sync is available: a share is set up (Phase 2's share setup). Screens show sync buttons and
 * options only when `sync.available` is true (the sync code itself is always there). It's live: it
 * follows a share being set up or removed while the app runs.
 *
 * Whether the platform can do something (Android Auto, background playback) is in `capabilities`
 * instead: that never changes while the app runs; this does.
 */
import { invoke } from '@tauri-apps/api/core'
import { reactive } from 'vue'
import { library } from '.'

export const sync = reactive({
  /** A share is set up: show the sync buttons and options. */
  available: false,
})

let started = false

/** Once at app start: reads it, then follows its changes. */
export async function startSync(): Promise<void> {
  if (started) return
  started = true
  await library.onChange((change) => {
    if (change.reason === 'sync') refresh().catch(() => {})
  })
  await refresh()
}

async function refresh() {
  sync.available = await invoke<boolean>('library_sync_available')
}
