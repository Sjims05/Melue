/**
 * Moving around with the arrows (a computer), like a TV app or a controller. Two places:
 * - the pages column (the dots; where Melue starts, and where a page switch leaves you): ↑ / ↓ the page above / below,
 *   → into the page, where you were on it last (else its first thing in view);
 * - the page: the arrows go to the nearest thing that way (songs, cards, buttons, tabs, settings); in a list of songs
 *   ↑ / ↓ go row by row (a long list draws only its rows on screen: scrolled on to draw the next), on past its ends;
 *   ← with nothing further left, or Esc with nothing to close, back out to the pages column. In a song row, → goes
 *   into its buttons (play, the title, artist and album, like, ⋯, ✕): ← / → between them, ← from the first back to
 *   the row, ↑ / ↓ the row above / below (as a whole again).
 * Shift+↑ / ↓ change page from anywhere. Passed over: what's marked `data-arrows-skip` (Now playing's player controls:
 * their keys do them). What has arrows of its own keeps them (a menu, a field, a slider, segments,
 * tabs across); a button with a menu (⋯) opens it with Enter, Space or M (↓ moves on). Everything scrolls into view
 * smoothly. A page opened on top (an artist, Search) has no pages column: the arrows go into it, Esc goes back.
 */
import { router, MAIN_PAGES } from '@/router'

/** What the arrows stop on: what can be used, a song row or a card as one (not the buttons in it). */
const STOPS =
  'a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select, textarea, [role=listitem], [role=tab], [role=switch], [role=checkbox], [role=slider], [tabindex]:not([tabindex="-1"])'
/** Things moved between as one, the buttons in them left out. */
const UNITS = '[role=listitem], .cover-card'

/** The page in view: the page opened on top, else the main page (the last scroller shown). */
export function pageArea(): HTMLElement | null {
  const all = [...document.querySelectorAll<HTMLElement>('[data-scroller]')].filter((el) => el.offsetParent && !el.closest('[inert]'))
  return all[all.length - 1] ?? null
}

const dots = () => document.querySelector<HTMLElement>('nav[aria-label=Pages]')
const onMainPage = () => !!router.currentRoute.value.meta.main

function shown(el: HTMLElement): boolean {
  if (el.closest('[inert], [aria-hidden=true]')) return false
  if (!el.getClientRects().length) return false
  return getComputedStyle(el).visibility !== 'hidden'
}

/** The page's stops. */
function stops(page: HTMLElement): HTMLElement[] {
  return [...page.querySelectorAll<HTMLElement>(STOPS)].filter((el) => {
    const unit = el.parentElement?.closest<HTMLElement>(UNITS)
    if (unit && page.contains(unit)) return false
    if (el.closest('[data-arrows-skip]')) return false
    // Segments: the chosen one stands for them (they have their own arrows).
    if (el.getAttribute('role') === 'radio' && el.getAttribute('tabindex') === '-1') return false
    return shown(el)
  })
}

/** Goes to `el`: focused, and scrolled into view smoothly. */
export function goTo(el: HTMLElement) {
  el.focus({ preventScroll: true })
  el.scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block: 'nearest', inline: 'nearest' })
}

type Direction = 'up' | 'down' | 'left' | 'right'

/**
 * The nearest stop that way: nearest along it, lined up across it counting more than a little off to the side, and of
 * those lined up, the one starting nearest where this one starts (under a title: its first button, as one reads).
 */
function nearest(from: HTMLElement, direction: Direction, candidates: HTMLElement[]): HTMLElement | undefined {
  const a = from.getBoundingClientRect()
  const vertical = direction === 'up' || direction === 'down'
  let best: HTMLElement | undefined
  let bestScore = Infinity
  for (const el of candidates) {
    if (el === from || el.contains(from) || from.contains(el)) continue
    const b = el.getBoundingClientRect()
    const along =
      direction === 'down' ? b.top - a.bottom : direction === 'up' ? a.top - b.bottom : direction === 'right' ? b.left - a.right : a.left - b.right
    // Not that way (a little overlap allowed: rows and cards touch).
    if (along < -4) continue
    const [aStart, aEnd, bStart, bEnd] = vertical ? [a.left, a.right, b.left, b.right] : [a.top, a.bottom, b.top, b.bottom]
    const across = Math.max(0, bStart - aEnd, aStart - bEnd)
    const starts = Math.abs(aStart - bStart)
    const score = Math.max(0, along) + across * 3 + starts * 0.05
    if (score < bestScore) {
      bestScore = score
      best = el
    }
  }
  return best
}

// ---- Where you were on each page (by its address), for going in again.
const lastSpot = new Map<string, HTMLElement>()
/** How many pages' places are kept. */
const REMEMBERED = 20
function remember(e: FocusEvent) {
  const el = e.target as HTMLElement | null
  const page = pageArea()
  if (!el || !page?.contains(el)) return
  const path = router.currentRoute.value.fullPath
  // The latest pages only (a page kept for coming back is out of the document meanwhile: that can't tell which are
  // gone for good), newest last.
  lastSpot.delete(path)
  lastSpot.set(path, el)
  if (lastSpot.size > REMEMBERED) lastSpot.delete(lastSpot.keys().next().value!)
}

/** Into the page: where you were on it last, else its first stop in view (nearest its top left). */
function enterPage() {
  const page = pageArea()
  if (!page) return
  const spot = lastSpot.get(router.currentRoute.value.fullPath)
  if (spot?.isConnected && page.contains(spot) && shown(spot)) return goTo(spot)
  const all = stops(page)
  const view = page.getBoundingClientRect()
  const inView = all.filter((el) => el.getBoundingClientRect().bottom > view.top + 1)
  // The one nearest the page's top left (being high counting double): in a page of columns, the first one's top
  // (Now playing: the song's artist, not the lyrics' buttons at the far right).
  const far = (el: HTMLElement) => {
    const box = el.getBoundingClientRect()
    return Math.max(0, box.top - view.top) * 2 + Math.max(0, box.left - view.left)
  }
  const first = (inView.length ? inView : all).sort((x, y) => far(x) - far(y))[0]
  if (first) goTo(first)
}

/** Out to the pages column: nothing in the page selected (the arrows change page again). */
function leavePage() {
  ;(document.activeElement as HTMLElement | null)?.blur()
}

// The main page shown: under a page on top, the one it was opened from (as the dots have it).
let mainPath = '/'
router.afterEach((to) => {
  if (to.meta.main) mainPath = to.path
})

/** The main page above / below (`by`), from wherever you are. */
function changePage(by: number) {
  const at = MAIN_PAGES.findIndex((p) => p.path === mainPath)
  const to = MAIN_PAGES[(at < 0 ? 0 : at) + by]
  if (to) router.push(to.path)
}

// ---- A list of songs: row by row (the next row may not be drawn yet: scrolled on a row, then looked for again).
function listStep(item: HTMLElement, list: HTMLElement, by: number): boolean {
  const items = () => [...list.querySelectorAll<HTMLElement>('[role=listitem]')].filter((i) => i.closest('[role=list]') === list && shown(i))
  const all = items()
  const next = all[all.indexOf(item) + by]
  if (next) {
    goTo(next)
    return true
  }
  const scroller = list.closest<HTMLElement>('[data-scroller]')
  if (!scroller) return false
  const box = item.getBoundingClientRect()
  const listBox = list.getBoundingClientRect()
  // At the list's real end: on to what's past it.
  if (by > 0 ? listBox.bottom - box.bottom < 2 : box.top - listBox.top < 2) return false
  scroller.scrollBy({ top: by * item.offsetHeight })
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      const again = items().find((i) => i !== item && (by > 0 ? i.getBoundingClientRect().top >= box.bottom - by * item.offsetHeight - 2 : i.getBoundingClientRect().bottom <= box.top - by * item.offsetHeight + 2))
      if (again) goTo(again)
    }),
  )
  return true
}

/** A song row's own stops (its buttons and links), left to right. */
function rowStops(row: HTMLElement): HTMLElement[] {
  return [...row.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), [role=button]')]
    .filter((el) => shown(el) && el.closest('[role=listitem]') === row)
    .sort((a, b) => a.getBoundingClientRect().left - b.getBoundingClientRect().left)
}

/** What keeps its own arrows (all four, or across only). */
function ownArrows(el: HTMLElement, key: string): boolean {
  if (el.closest('input, textarea, select, [contenteditable], [role=menu], [role=listbox], [role=dialog], [role=alertdialog], [role=slider], [role=radiogroup], [data-own-keys]'))
    return true
  // Tabs: across between them (↑ / ↓ move on).
  if (el.closest('[role=tablist]') && (key === 'ArrowLeft' || key === 'ArrowRight')) return true
  return false
}

const DIRECTIONS: Record<string, Direction> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' }

/** The arrows (first, before what's selected: a ⋯ button's ↓ would open its menu), and Esc out of a page. */
function arrowKey(e: KeyboardEvent) {
  if (e.defaultPrevented || e.ctrlKey || e.altKey || e.metaKey) return
  if (document.querySelector('[role=dialog], [role=alertdialog], [role=menu]')) return
  const active = document.activeElement as HTMLElement | null
  const page = pageArea()
  const inPage = !!(active && page?.contains(active))

  // A song picked up to move (Shift+Enter): its list has the keys (SongList.vue).
  if (active?.closest('[data-moving]')) return
  if (e.key === 'Escape') {
    if (!e.shiftKey && inPage && onMainPage() && !active!.closest('input, textarea, [contenteditable]')) {
      e.preventDefault()
      leavePage()
    }
    return
  }
  const direction = DIRECTIONS[e.key]
  if (!direction) return

  // Shift+↑ / ↓: the page above / below, from anywhere (← / →: the seek, lib/hotkeys.ts).
  if (e.shiftKey) {
    if (direction === 'up' || direction === 'down') {
      e.preventDefault()
      leavePage()
      changePage(direction === 'down' ? 1 : -1)
    }
    return
  }

  // The pages column (or a page on top, which has none: the arrows go into it).
  if (!inPage) {
    if (active?.closest('input, textarea, [contenteditable], [role=slider]')) return
    e.preventDefault()
    if (onMainPage() && dots() && (direction === 'up' || direction === 'down')) {
      ;(document.activeElement as HTMLElement | null)?.blur()
      changePage(direction === 'down' ? 1 : -1)
    } else if (direction === 'right' || !onMainPage()) enterPage()
    return
  }

  if (ownArrows(active!, e.key)) return
  // Taken here: what's selected (a ⋯ button, a list) doesn't act on it too.
  e.preventDefault()
  e.stopPropagation()
  const item = active!.closest<HTMLElement>('[role=listitem]')
  const list = item?.closest<HTMLElement>('[role=list]')
  // In a song row's buttons: across between them, ← from the first back to the row; ↑ / ↓ go on from the row.
  if (item && active !== item && item.hasAttribute('data-song')) {
    if (direction === 'left' || direction === 'right') {
      const own = rowStops(item)
      const at = own.indexOf(active!)
      const to = own[at + (direction === 'right' ? 1 : -1)]
      return goTo(to ?? (direction === 'left' ? item : active!))
    }
    if (list && listStep(item, list, direction === 'down' ? 1 : -1)) return
  }
  // A song row: → into its buttons.
  if (item && active === item && item.hasAttribute('data-song') && direction === 'right') {
    const first = rowStops(item)[0]
    if (first) return goTo(first)
  }
  if (item && list && (direction === 'up' || direction === 'down') && listStep(item, list, direction === 'down' ? 1 : -1)) return
  const to = nearest(active!, direction, stops(page!))
  if (to) goTo(to)
  else if (direction === 'left' && onMainPage() && dots()) leavePage()
}

let started = false
/** Once (the computer's frame, with the other shortcuts). */
export function startArrowNav() {
  if (started) return
  started = true
  window.addEventListener('keydown', arrowKey, true)
  document.addEventListener('focusin', remember)
}

if (import.meta.hot) {
  if (import.meta.hot.data.started) startArrowNav()
  import.meta.hot.dispose((data) => {
    data.started = started
    window.removeEventListener('keydown', arrowKey, true)
    document.removeEventListener('focusin', remember)
  })
}
