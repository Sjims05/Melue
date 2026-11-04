/**
 * Moving around with the arrows (a computer), like a TV app or a controller. Two places:
 * - the pages column (the dots; where Melue starts, and where a page switch leaves you): ↑ / ↓ the page above / below,
 *   → into the page, where you were on it last (else its first thing in view);
 * - the page: the arrows go to the nearest thing that way (songs, cards, buttons, tabs, settings); in a list of songs
 *   ↑ / ↓ go row by row (a long list draws only its rows on screen: scrolled on to draw the next), on past its ends;
 *   ← with nothing further left, or Esc with nothing to close, back out to the pages column. In a song row, → goes
 *   into its buttons (play, the title, artist and album, like, ⋯, ✕): ← / → between them, ← from the first back to
 *   the row, ↑ / ↓ the row above / below (as a whole again). A settings row likewise: ← / → only between its own
 *   controls, ← past the first out to the pages column.
 * Shift+↑ / ↓ change page from anywhere; Ctrl+↑ / ↓ scroll, from anywhere. Passed over: what's marked `data-arrows-skip` (Now playing's player controls:
 * their keys do them). What has arrows of its own keeps them (a menu, a field, a slider, segments,
 * tabs across); a button with a menu (⋯) opens it with Enter, Space or M (↓ moves on). Everything scrolls into view
 * smoothly, by a scroll that follows the selection (an arrow held down: steadily, not restarted each step). A page
 * opened on top (an artist, Search) has no pages column: the arrows go into it, Esc goes back. An overlay marked
 * `data-arrows-page` (the first-start guide, New in Melue) is the page while it's open: the arrows stay in it, and the
 * pages behind it don't change.
 */
import { router, MAIN_PAGES } from '@/router'

/** What the arrows stop on: what can be used, a song row or a card as one (not the buttons in it). */
const STOPS =
  'a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select, textarea, [role=listitem], [role=tab], [role=switch], [role=checkbox], [role=slider], [tabindex]:not([tabindex="-1"])'
/** Things moved between as one, the buttons in them left out. */
const UNITS = '[role=listitem], .cover-card'

/** An overlay that is the page while it's open (the first-start guide, New in Melue). */
const overlay = () => document.querySelector<HTMLElement>('[data-arrows-page]')

/** The page in view: an overlay, the page opened on top, else the main page (the last scroller shown). */
export function pageArea(): HTMLElement | null {
  const over = overlay()
  if (over) return over
  const all = [...document.querySelectorAll<HTMLElement>('[data-scroller]')].filter((el) => el.offsetParent && !el.closest('[inert]'))
  return all[all.length - 1] ?? null
}

const dots = () => document.querySelector<HTMLElement>('nav[aria-label=Pages]')
const onMainPage = () => !!router.currentRoute.value.meta.main && !overlay()

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

// ---- The scroll that follows the selection: up and down in the page, eased toward where the selection needs it, the
// aim moved on as the selection moves (smooth scrolls restarted many times a second, an arrow held down, fell behind
// and jumped). Across, and in anything else that scrolls: the browser's own.
const aims = new Map<HTMLElement, number>()
/** Each frame, this much of the way left (at 60 frames a second). */
const EASE = 0.25

function follow(scroller: HTMLElement, aim: number) {
  const max = scroller.scrollHeight - scroller.clientHeight
  aim = Math.max(0, Math.min(max, aim))
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return void (scroller.scrollTop = aim)
  const running = aims.has(scroller)
  aims.set(scroller, aim)
  if (running) return
  let last = performance.now()
  const frame = (now: number) => {
    const to = aims.get(scroller)!
    const left = to - scroller.scrollTop
    if (Math.abs(left) < 1) {
      scroller.scrollTop = to
      return void aims.delete(scroller)
    }
    scroller.scrollTop += left * (1 - Math.pow(1 - EASE, (now - last) / 16.7))
    last = now
    requestAnimationFrame(frame)
  }
  requestAnimationFrame(frame)
}

/** Scrolled by `by` now (a long list's next rows drawn), the scroll following the selection moved along with it. */
function scrollNow(scroller: HTMLElement, by: number) {
  scroller.scrollTop += by
  const aim = aims.get(scroller)
  if (aim !== undefined) aims.set(scroller, aim + by)
}

/** Goes to `el`: focused, and scrolled into view smoothly (`room`: with room past it). */
export function goTo(el: HTMLElement, room = true) {
  // A group of segments: straight to its chosen one (the group hands its focus on with a jump of the page's own).
  if (el.matches('[role=group], [role=radiogroup]'))
    el = el.querySelector<HTMLElement>('[data-state=on], [data-state=checked], [aria-pressed=true], [aria-checked=true]') ?? el
  el.focus({ preventScroll: true })
  const scroller = el.closest<HTMLElement>('[data-scroller], [data-arrows-page]')
  const asBrowser: ScrollIntoViewOptions = { behavior: 'smooth', block: 'nearest', inline: 'nearest' }
  if (!scroller) return el.scrollIntoView(asBrowser)
  // Up and down: from where it's heading, enough to show it with room past it (what comes next shows ahead, and the
  // page's top, its title, comes in bit by bit rather than all at the last step).
  const box = el.getBoundingClientRect()
  const view = scroller.getBoundingClientRect()
  const from = aims.get(scroller) ?? scroller.scrollTop
  const past = room ? Math.min(box.height * 1.5, scroller.clientHeight / 4) : 0
  const margin = parseFloat(getComputedStyle(el).scrollMarginTop) || 0
  const top = box.top - view.top + scroller.scrollTop - margin - past
  const bottom = box.bottom - view.top + scroller.scrollTop + past
  // Within that room of the top: all the way up.
  if (top < from) follow(scroller, top < past ? 0 : top)
  else if (bottom > from + scroller.clientHeight) follow(scroller, Math.min(top, bottom - scroller.clientHeight))
  // Across (a row of cards), and anything scrolling inside the page: as the browser does it.
  const inner = el.parentElement?.closest<HTMLElement>('[data-scroller], [data-arrows-page], .overflow-x-auto, .overflow-auto')
  if (inner && inner !== scroller) el.scrollIntoView(asBrowser)
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

/** Whether `el` is in the page's view (`whole`: all of it). */
function inView(el: HTMLElement, page: HTMLElement, whole = false): boolean {
  const box = el.getBoundingClientRect()
  const view = page.getBoundingClientRect()
  return whole ? box.top >= view.top - 1 && box.bottom <= view.bottom + 1 : box.bottom > view.top + 1 && box.top < view.bottom - 1
}

/** Into the page: where you were on it last while that's in view (not scrolled away from), else its first stop in view
 * (nearest its top left). */
function enterPage() {
  const page = pageArea()
  if (!page) return
  const spot = lastSpot.get(router.currentRoute.value.fullPath)
  if (spot?.isConnected && page.contains(spot) && shown(spot) && inView(spot, page)) return goTo(spot, false)
  const all = stops(page)
  const view = page.getBoundingClientRect()
  // Wholly in view if any are (nothing scrolls), else partly.
  const whole = all.filter((el) => inView(el, page, true))
  const seen = whole.length ? whole : all.filter((el) => inView(el, page))
  // The one nearest the page's top left (being high counting double): in a page of columns, the first one's top
  // (Now playing: the song's artist, not the lyrics' buttons at the far right).
  const far = (el: HTMLElement) => {
    const box = el.getBoundingClientRect()
    return Math.max(0, box.top - view.top) * 2 + Math.max(0, box.left - view.left)
  }
  const first = (seen.length ? seen : all).sort((x, y) => far(x) - far(y))[0]
  if (first) goTo(first, false)
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
  scrollNow(scroller, by * item.offsetHeight)
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      // The row after it, now drawn (else, gone itself: the first row past where it was).
      const now = items()
      const at = now.indexOf(item)
      const past = (i: HTMLElement) =>
        by > 0 ? i.getBoundingClientRect().top >= box.bottom - by * item.offsetHeight - 2 : i.getBoundingClientRect().bottom <= box.top - by * item.offsetHeight + 2
      const again = at >= 0 ? now[at + by] : now.find((i) => i !== item && past(i))
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

/** What Ctrl+↑ / ↓ scroll: the open dialog's scrolling part, else the page in view. */
function scrollable(): HTMLElement | null {
  const dialog = document.querySelector<HTMLElement>('[role=dialog]')
  if (!dialog) return pageArea()
  return [...dialog.querySelectorAll<HTMLElement>('*')].find((el) => el.scrollHeight > el.clientHeight + 2 && /(auto|scroll)/.test(getComputedStyle(el).overflowY)) ?? null
}

/**
 * Ctrl+↑ / ↓: the page (or a dialog's list) scrolled most of a screen, smoothly, from wherever the selection is (here,
 * first: a selected dropdown would open on ↓). Not in a text field (its own).
 */
function ctrlScroll(e: KeyboardEvent): boolean {
  if (!e.ctrlKey || e.shiftKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return false
  if ((e.target as HTMLElement | null)?.closest?.('input, textarea, select, [contenteditable]')) return false
  e.preventDefault()
  e.stopPropagation()
  const area = scrollable()
  if (area) follow(area, (aims.get(area) ?? area.scrollTop) + (e.key === 'ArrowDown' ? 1 : -1) * area.clientHeight * 0.6)
  return true
}

/** The arrows (first, before what's selected: a ⋯ button's ↓ would open its menu), and Esc out of a page. */
function arrowKey(e: KeyboardEvent) {
  if (e.defaultPrevented || e.altKey || e.metaKey) return
  if (ctrlScroll(e) || e.ctrlKey) return
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

  // Shift+↑ / ↓: the page above / below, from anywhere but an overlay (← / →: the seek, lib/hotkeys.ts).
  if (e.shiftKey) {
    if ((direction === 'up' || direction === 'down') && !overlay()) {
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
  // What's selected scrolled out of view (Ctrl+↑ / ↓): from what's in view instead, its first thing.
  if (!inView(active!, page!) && !active!.closest('[data-arrows-skip]')) {
    e.preventDefault()
    return enterPage()
  }
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
  // A settings row, as one (as a song row): ← / → between its own controls only, ← past its first out to the pages
  // column (not into another row's control that happens to sit further left).
  const row = active!.closest<HTMLElement>('[data-row]')
  if (row && page!.contains(row) && (direction === 'left' || direction === 'right')) {
    const own = stops(page!).filter((el) => row.contains(el))
    const to = nearest(active!, direction, own)
    if (to) goTo(to)
    else if (direction === 'left' && onMainPage() && dots()) leavePage()
    return
  }
  const to = nearest(active!, direction, stops(page!))
  if (to) goTo(to)
  else if (direction === 'left' && onMainPage() && dots()) leavePage()
}

/**
 * Enter or Space that left its button unusable (Back at the first step: disabled; or hidden): the selection, dropped
 * with it, goes on to the nearest thing above, as ↑ would (not left nowhere, the next key going to the pages column).
 */
function pressed(e: KeyboardEvent) {
  if (e.key !== 'Enter' && e.key !== ' ') return
  const was = document.activeElement as HTMLElement | null
  const page = pageArea()
  if (!was || !page?.contains(was)) return
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      const now = document.activeElement
      if (now && now !== document.body && now !== was) return
      if (!was.isConnected || !(was.matches(':disabled') || !shown(was))) return
      const to = nearest(was, 'up', stops(page))
      if (to) goTo(to)
    }),
  )
}

let started = false
/** Once (the computer's frame, with the other shortcuts). */
export function startArrowNav() {
  if (started) return
  started = true
  window.addEventListener('keydown', arrowKey, true)
  window.addEventListener('keydown', pressed)
  document.addEventListener('focusin', remember)
}

if (import.meta.hot) {
  if (import.meta.hot.data.started) startArrowNav()
  import.meta.hot.dispose((data) => {
    data.started = started
    window.removeEventListener('keydown', arrowKey, true)
    window.removeEventListener('keydown', pressed)
    document.removeEventListener('focusin', remember)
  })
}
