/**
 * What the app shows of a running sync (the status card, the top bar's sync status): held at least SHOWN_AT_LEAST ms,
 * and when it's done the bar fills and stays a moment (FILLED_FOR) before "Synced", so a quick sync reads as one, not a
 * flash. Only what's shown: syncing isn't slowed.
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { sync, type SyncProgress } from '@/library/sync'

const SHOWN_AT_LEAST = 1200
const FILLED_FOR = 400

/** The sync as shown: its progress while it runs (and a moment after), null otherwise. */
export function useShownSync() {
  const shown = ref<SyncProgress | null>(null)
  let since = 0
  let hide: ReturnType<typeof setTimeout> | undefined
  watch(
    () => sync.status?.syncing ?? null,
    (live) => {
      clearTimeout(hide)
      if (live) {
        if (!shown.value) since = Date.now()
        shown.value = live
        return
      }
      const last = shown.value
      if (!last) return
      shown.value = { ...last, filesDone: last.filesTotal, bytesDone: last.bytesTotal, current: null }
      hide = setTimeout(() => (shown.value = null), Math.max(0, SHOWN_AT_LEAST - (Date.now() - since)) + FILLED_FOR)
    },
    { immediate: true },
  )
  onBeforeUnmount(() => clearTimeout(hide))
  return computed(() => shown.value)
}
