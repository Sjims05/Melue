/**
 * A timer that runs only while its page is shown: not while the page is kept aside (pages stay alive once visited:
 * Settings' timers ran on for the rest of the session), nor while the window is hidden. Back in view, it runs `fn`
 * at once, then every `ms`.
 */
import { useDocumentVisibility, useIntervalFn } from '@vueuse/core'
import { onActivated, onDeactivated, ref, watch } from 'vue'

export function everyWhileShown(fn: () => unknown, ms: number) {
  const active = ref(true)
  const visible = useDocumentVisibility()
  const { pause, resume } = useIntervalFn(fn, ms)
  onActivated(() => (active.value = true))
  onDeactivated(() => (active.value = false))
  watch([active, visible], ([shown, seen], [wasShown, wasSeen]) => {
    const now = shown && seen === 'visible'
    const was = wasShown && wasSeen === 'visible'
    if (now === was) return
    if (!now) return pause()
    fn()
    resume()
  })
}
