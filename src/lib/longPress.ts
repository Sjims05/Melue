/**
 * A long press (a finger held still for a moment), for a phone's menus. Returns the handlers to put on the element;
 * a press that moved (a scroll, a drag) isn't one. The tap that ends a long press is swallowed wherever it lands: on
 * what was pressed it would open or play it, and on the menu that just opened under the finger it would pick a row.
 * Only touch counts: a mouse has its right-click menu. The handlers listen as the event comes down to the element
 * (capture), so they sit on the element itself next to a right-click menu's own touch handling, which replaces
 * handlers of the same name.
 */
const HOLD_MS = 450
const STILL_PX = 8

/**
 * Swallows the click that letting go of a long press makes, wherever it lands. Phones often send none after a long
 * press, so it only waits until the finger is lifted (and a moment), not for the next tap.
 */
let stopSwallowing: (() => void) | null = null
function swallowNextClick() {
  const swallow = (e: MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    stop()
  }
  const stop = () => {
    window.removeEventListener('click', swallow, true)
    stopSwallowing = null
  }
  window.addEventListener('click', swallow, true)
  stopSwallowing = stop
}
const lifted = () => {
  const stop = stopSwallowing
  if (stop) setTimeout(stop, 100)
}

export function useLongPress(fire: (e: PointerEvent) => unknown) {
  let timer: ReturnType<typeof setTimeout> | undefined
  let start: { x: number; y: number } | null = null
  let fired = false
  const stop = () => {
    clearTimeout(timer)
    start = null
  }
  return {
    onPointerdownCapture(e: PointerEvent) {
      fired = false
      if (e.pointerType !== 'touch') return
      start = { x: e.clientX, y: e.clientY }
      timer = setTimeout(() => {
        fired = true
        start = null
        navigator.vibrate?.(10)
        swallowNextClick()
        fire(e)
      }, HOLD_MS)
    },
    onPointermoveCapture(e: PointerEvent) {
      if (start && Math.hypot(e.clientX - start.x, e.clientY - start.y) > STILL_PX) stop()
    },
    onPointerupCapture() {
      stop()
      if (fired) lifted()
    },
    onPointercancelCapture() {
      stop()
      if (fired) lifted()
    },
    // The browser's own long-press menu (copy, open link): not for these.
    onContextmenuCapture(e: Event) {
      if (start || fired) e.preventDefault()
    },
  }
}
