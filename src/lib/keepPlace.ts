/**
 * `v-keep-place` on a page's scroller: when what's above the view changes height (the window resized: text wraps
 * differently, a cover gets smaller; or anything else above it grows or shrinks), what you were looking at stays where
 * it is. That's the first song row in view (the songs stay put, a header above them shrinks out of the way), else the
 * first thing that starts in the view (going into a card across its top: in Settings, a sync line changing in a card
 * above doesn't move the rows you're reading). It scrolls by however far that moved, before the frame is drawn. At the
 * very top it stays at the top.
 *
 * The browser's own scroll anchoring doesn't do it here: it's off on the main pages' scroller (a page switch would
 * throw it off, lessons in the playbook), and long lists only draw the rows in view. A page switch takes its anchor
 * out of the page (it's in the old one), so nothing is corrected then.
 */
import type { Directive } from 'vue'

type Place = {
  anchor: Element | null
  top: number
  picking: number
  sizes: ResizeObserver
  scrolled: () => void
  /** The scroller's parts watched for size (each element in it: on a phone, pull to refresh's mark comes first). */
  watched: Set<Element>
  /** The scroll this set itself (its own scroll event isn't you scrolling). */
  own: number | null
}
const places = new WeakMap<HTMLElement, Place>()

/** What to hold still, and where it is now: the first song row in view, else what's at the top of the view. */
function pick(el: HTMLElement, place: Place) {
  const view = el.getBoundingClientRect()
  // The rows are in the page's order, top to bottom: the first one reaching into the view, found by halving.
  const rows = el.querySelectorAll('[data-song-row]')
  let low = 0
  let high = rows.length
  while (low < high) {
    const mid = (low + high) >> 1
    if (rows[mid].getBoundingClientRect().bottom > view.top + 1) high = mid
    else low = mid + 1
  }
  const anchor: Element | null = rows[low] && rows[low].getBoundingClientRect().top < view.bottom ? rows[low] : firstInView(el, view)
  place.anchor = anchor
  place.top = anchor ? anchor.getBoundingClientRect().top - view.top : 0
}

/**
 * The first thing that starts in the view (as the browser's own scroll anchoring picks): down the page's parts in order,
 * the first whose top is in the view; one across the view's top is gone into, for a smaller part of it (else itself).
 */
function firstInView(parent: Element, view: DOMRect): Element | null {
  for (const part of parent.children) {
    const box = part.getBoundingClientRect()
    if (box.height === 0 || box.bottom <= view.top) continue
    if (box.top >= view.bottom) return null
    if (box.top >= view.top) return part
    return firstInView(part, view) ?? part
  }
  return null
}

/**
 * Back where it was (the layout above it changed). Its place is kept as noted when you last scrolled, not noted again
 * here: a header shrinking a fraction of a pixel a step would otherwise add up, the scroll being in whole pixels.
 */
function hold(el: HTMLElement, place: Place) {
  const anchor = place.anchor
  const held = el.scrollTop > 0 && !!anchor?.isConnected
  if (held) {
    const moved = anchor!.getBoundingClientRect().top - el.getBoundingClientRect().top - place.top
    if (Math.abs(moved) >= 0.5) {
      el.scrollTop += moved
      place.own = el.scrollTop
    }
  }
  // Every part of the scroller watched (new ones too; gone ones let go).
  for (const part of place.watched) {
    if (part.parentElement === el) continue
    place.sizes.unobserve(part)
    place.watched.delete(part)
  }
  for (const part of el.children) {
    if (place.watched.has(part)) continue
    place.sizes.observe(part)
    place.watched.add(part)
  }
  if (!held) pick(el, place)
}

export const vKeepPlace: Directive<HTMLElement> = {
  mounted(el) {
    const place: Place = {
      anchor: null,
      top: 0,
      picking: 0,
      watched: new Set(),
      own: null,
      // Once a frame at most, while scrolling (not after its own correction).
      scrolled: () => {
        if (el.scrollTop === place.own) return
        place.own = null
        if (!place.picking) place.picking = requestAnimationFrame(() => ((place.picking = 0), pick(el, place)))
      },
      // The scroller itself (the window's size) and its content (anything in the page growing or shrinking).
      sizes: new ResizeObserver(() => hold(el, place)),
    }
    places.set(el, place)
    el.addEventListener('scroll', place.scrolled, { passive: true })
    place.sizes.observe(el)
    hold(el, place)
  },
  unmounted(el) {
    const place = places.get(el)
    if (!place) return
    cancelAnimationFrame(place.picking)
    place.sizes.disconnect()
    el.removeEventListener('scroll', place.scrolled)
    places.delete(el)
  },
}
