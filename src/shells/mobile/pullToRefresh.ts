/**
 * Pulling a main page down from its top to bring everything up to date (the phone's usual gesture): a sync with the
 * share (the music, likes, playlists, settings), or without a share, a scan of the music folders. The page doesn't
 * move; a round mark comes down with the finger (`pull`, in pixels, slowed down), turning as it comes, and let go past
 * `READY` it stays and spins (`refreshing`) until it's done. Starts only on a page scrolled to its top; sliders, drag
 * handles and text fields keep their own drags.
 */
import { onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { library } from '@/library'
import { describeSyncFailure, sync, syncing, type SyncFailure } from '@/library/sync'
import { toast } from '@/lib/toast'

const OWN_DRAG = '[role=slider], .song-handle, input, textarea, [data-no-pull]'
/** How far the mark comes down before letting go refreshes, and the furthest it comes. */
export const READY = 72
const FURTHEST = 110
/** Spinning at least this long: a quick one would only flash. */
const AT_LEAST_MS = 600

async function refresh() {
  if (!sync.available) return void (await library.scan())
  try {
    await syncing.now()
  } catch (e) {
    const kind = (e as SyncFailure)?.kind
    if (kind !== 'cancelled' && kind !== 'running') toast(`Can't sync: ${describeSyncFailure(e)}`)
  }
}

export function usePullToRefresh(scroller: Ref<HTMLElement | null | undefined>) {
  const pull = ref(0)
  const pulling = ref(false)
  const refreshing = ref(false)
  let start: { x: number; y: number } | null = null
  let mine: boolean | null = null

  function down(e: TouchEvent) {
    const el = scroller.value
    const ok = el && !refreshing.value && e.touches.length === 1 && el.scrollTop <= 0 && !(e.target as HTMLElement).closest(OWN_DRAG)
    start = ok ? { x: e.touches[0].clientX, y: e.touches[0].clientY } : null
    mine = null
  }
  function move(e: TouchEvent) {
    if (!start) return
    const dx = e.touches[0].clientX - start.x
    const dy = e.touches[0].clientY - start.y
    if (mine === null) {
      if (Math.abs(dx) < 8 && Math.abs(dy) < 8) return
      mine = dy > 0 && Math.abs(dy) > Math.abs(dx) && (scroller.value?.scrollTop ?? 1) <= 0
      if (!mine) return void (start = null)
      pulling.value = true
    }
    e.preventDefault()
    const before = pull.value
    // Half as fast as the finger, up to the furthest.
    pull.value = Math.min(FURTHEST, Math.max(0, dy) * 0.5)
    if (before < READY && pull.value >= READY) navigator.vibrate?.(10)
  }
  async function up() {
    const was = mine
    start = null
    mine = null
    if (!was) return
    pulling.value = false
    if (pull.value < READY) return void (pull.value = 0)
    pull.value = READY
    refreshing.value = true
    const began = performance.now()
    await refresh().catch((e) => toast(`Can't refresh: ${String(e)}`))
    await new Promise((done) => setTimeout(done, Math.max(0, AT_LEAST_MS - (performance.now() - began))))
    refreshing.value = false
    pull.value = 0
  }

  // A move can stop the page scrolling: not passive.
  let listening: HTMLElement | null = null
  function listen(el: HTMLElement | null | undefined) {
    if (listening === el) return
    if (listening) {
      listening.removeEventListener('touchstart', down)
      listening.removeEventListener('touchmove', move)
      listening.removeEventListener('touchend', up)
      listening.removeEventListener('touchcancel', up)
    }
    listening = el ?? null
    if (!el) return
    el.addEventListener('touchstart', down, { passive: true })
    el.addEventListener('touchmove', move, { passive: false })
    el.addEventListener('touchend', up)
    el.addEventListener('touchcancel', up)
  }
  watch(scroller, listen, { immediate: true })
  onBeforeUnmount(() => listen(null))

  return { pull, pulling, refreshing }
}
