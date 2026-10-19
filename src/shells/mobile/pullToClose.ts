/**
 * Closing a phone screen by pulling it away with a finger, from anywhere on it: Now playing down, a page opened on top
 * to the right. It only takes the gesture when nothing under the finger would move that way itself: Now playing only
 * pulls down once what's under the finger is scrolled to its top; a page only goes right if nothing under the finger
 * scrolls sideways. Sliders, drag handles and text fields keep their own drags. The screen follows the finger
 * (`pull`, in pixels); let go far enough, or flicked, and it closes; else it goes back.
 */
import { onBeforeUnmount, ref, watch, type Ref } from 'vue'

type Way = 'down' | 'right'
const OWN_DRAG = '[role=slider], .song-handle, input, textarea, [data-no-pull]'

function scrollsAway(from: HTMLElement, upTo: HTMLElement, way: Way) {
  for (let el: HTMLElement | null = from; el && el !== upTo.parentElement; el = el.parentElement) {
    const style = getComputedStyle(el)
    if (way === 'down') {
      const scrolls = /(auto|scroll)/.test(style.overflowY) && el.scrollHeight > el.clientHeight
      if (scrolls && el.scrollTop > 0) return true
    } else if (/(auto|scroll)/.test(style.overflowX) && el.scrollWidth > el.clientWidth) return true
  }
  return false
}

export function usePullToClose(target: Ref<HTMLElement | null | undefined>, way: Way, close: () => void) {
  const pull = ref(0)
  const pulling = ref(false)
  let start: { x: number; y: number; at: number } | null = null
  let mine: boolean | null = null

  function down(e: TouchEvent) {
    const el = target.value
    if (!el || e.touches.length > 1 || (e.target as HTMLElement).closest(OWN_DRAG)) return void (start = null)
    start = { x: e.touches[0].clientX, y: e.touches[0].clientY, at: performance.now() }
    mine = null
  }
  function move(e: TouchEvent) {
    const el = target.value
    if (!start || !el) return
    const dx = e.touches[0].clientX - start.x
    const dy = e.touches[0].clientY - start.y
    if (mine === null) {
      if (Math.abs(dx) < 8 && Math.abs(dy) < 8) return
      const along = way === 'down' ? dy > 0 && Math.abs(dy) > Math.abs(dx) : dx > 0 && Math.abs(dx) > Math.abs(dy) * 1.2
      mine = along && !scrollsAway(e.target as HTMLElement, el, way)
      if (mine) pulling.value = true
    }
    if (!mine) return
    e.preventDefault()
    pull.value = Math.max(0, way === 'down' ? dy : dx)
  }
  function up() {
    const el = target.value
    if (!start || !mine || !el) return void (start = null)
    const speed = pull.value / (performance.now() - start.at)
    const far = pull.value > (way === 'down' ? 140 : el.clientWidth * 0.33)
    start = null
    mine = null
    pulling.value = false
    if (far || (speed > 0.5 && pull.value > 40)) close()
    else pull.value = 0
  }

  // Listened to on the screen itself (it can come and go): a move can stop the page scrolling (not passive).
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
  watch(target, listen, { immediate: true })
  onBeforeUnmount(() => listen(null))

  /** Back where it was (after it closed: the next one starts fresh). */
  const reset = () => (pull.value = 0)
  return { pull, pulling, reset }
}
