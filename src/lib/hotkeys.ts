/**
 * Keyboard shortcuts (a computer; the list in KeyboardShortcuts.vue, opened with ?): the player and the pages, from
 * anywhere except while typing in a field. Where something has keys of its own (a list, a slider, segments, a menu)
 * it keeps the arrows, Space and Enter; the letters and Ctrl combinations still work there. A menu or a dialog open:
 * its own keys work in it (the arrows scroll the shortcuts list), and an action key closes it first, then acts; a
 * question (role=alertdialog: "Delete …?") has all the keys to itself. Keys with the Windows key or Alt are the
 * system's. Also: Ctrl+↑ / ↓ scroll (smoothly), Tab stays within the page (not the top bar or the player bar), and
 * a page switch puts the focus back at the page's top. The arrows move around the pages and in them (arrowNav.ts),
 * Escape goes back (DesktopShell.vue), Ctrl+Space and / open Search (TopSearch.vue).
 */
import { reactive } from 'vue'
import { router, MAIN_PAGES } from '@/router'
import { current, cycleRepeat, like, next, nowPlaying, position, previous, seek, setVolume, toggleShuffle, togglePlay, volumeStep } from '@/player/nowPlaying'
import { upNextOpen } from '@/player/upNext'
import { openReport } from '@/lib/report/report'
import { goTo, pageArea, startArrowNav } from '@/lib/arrowNav'

/** The shortcuts, as the list shows them (`keys`: each a key, or keys pressed together). */
export const SHORTCUTS: { group: string; items: { keys: string[][]; what: string }[] }[] = [
  {
    group: 'Playing',
    items: [
      { keys: [['Space']], what: 'Play / pause' },
      { keys: [['Shift', '←'], ['Shift', '→']], what: 'Back / forward 5 seconds (held: on and on)' },
      { keys: [['Ctrl', '←'], ['Ctrl', '→']], what: 'Previous / next song' },
      { keys: [['+'], ['−']], what: 'Volume up / down' },
      { keys: [['L']], what: 'Like the song playing' },
      { keys: [['S']], what: 'Shuffle' },
      { keys: [['R']], what: 'Repeat' },
    ],
  },
  {
    group: 'Going places',
    items: [
      { keys: [['↑'], ['↓']], what: 'The page above / below (Home, Library, Playlists, Now playing, Settings)' },
      { keys: [['→']], what: 'Into the page (where you were on it)' },
      { keys: [['←'], ['Esc']], what: 'Back out to the pages (← from the page’s left edge)' },
      { keys: [['Shift', '↑'], ['Shift', '↓']], what: 'The page above / below, from anywhere' },
      { keys: [['Ctrl', '1–5']], what: 'Those pages, by number' },
      { keys: [['Ctrl', '↑'], ['Ctrl', '↓']], what: 'Scroll up / down' },
      { keys: [['Ctrl', 'Space'], ['/']], what: 'Search' },
      { keys: [['Q']], what: 'Up next (again: closed, back where you were)' },
      { keys: [['Y']], what: 'Lyrics full size (again: closed)' },
      { keys: [['Esc']], what: 'Close, or back' },
      { keys: [['?']], what: 'These shortcuts' },
      { keys: [['Ctrl', 'Shift', 'B']], what: 'Report a bug' },
    ],
  },
  {
    group: 'In a page',
    items: [
      { keys: [['↑'], ['↓'], ['←'], ['→']], what: 'The nearest thing that way: a song, a card, a button, a setting' },
      { keys: [['Enter']], what: 'Play the song, open the card' },
      { keys: [['Shift', 'Enter']], what: 'Pick a song up to move it (Up next, a playlist): ↑ / ↓, then Enter (Esc: back)' },
      { keys: [['M']], what: 'The menu of what’s selected (also Shift+F10, the Menu key)' },
    ],
  },
]

/** The list of shortcuts is open. */
export const shortcuts = reactive({ open: false })
/** The volume keys were used (when): the player bar shows the volume a moment (VolumeButton.vue). */
export const volumeFlash = reactive({ at: 0 })

/** Where the arrows, Space and Enter belong to something else: a list, a slider, segments, a menu, a dialog. */
function ownsArrows(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  return !!el?.closest?.(
    '[role=slider], [role=radiogroup], [role=tablist], [role=menu], [role=listbox], [role=list], .cover-grid, [role=dialog], [role=alertdialog], [aria-haspopup], [data-own-keys]',
  )
}

/** What's open over the page: a question (only its keys), a dialog or a menu (an action closes it), or nothing. */
function opened(): 'question' | 'layer' | null {
  if (document.querySelector('[role=alertdialog]')) return 'question'
  if (document.querySelector('[role=dialog], [role=menu]')) return 'layer'
  return null
}

/** Closes the dialog or menu open (as Escape does). */
function closeLayer() {
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true }))
}

/** What Ctrl+↑ / ↓ scroll: the open dialog's scrolling part, else the page in view. */
function scrollable(): HTMLElement | null {
  const dialog = document.querySelector<HTMLElement>('[role=dialog]')
  if (!dialog) return pageArea()
  return [...dialog.querySelectorAll<HTMLElement>('*')].find((el) => el.scrollHeight > el.clientHeight + 2 && /(auto|scroll)/.test(getComputedStyle(el).overflowY)) ?? null
}

// ---- Q: Up next, from anywhere, and back where you were.
let upNextFrom: { path: string; el: HTMLElement | null } | null = null
function toggleUpNext() {
  const here = router.currentRoute.value
  if (here.name === 'now-playing' && upNextOpen.value) {
    upNextOpen.value = false
    const from = upNextFrom
    upNextFrom = null
    if (from && from.path !== here.fullPath) router.push(from.path).then(() => setTimeout(() => from.el?.focus({ preventScroll: true }), 400))
    else from?.el?.focus({ preventScroll: true })
    return
  }
  upNextFrom = { path: here.fullPath, el: document.activeElement as HTMLElement | null }
  router.push({ path: '/now-playing', query: { queue: 'open' } })
  // Once it's open and in view: its first song.
  setTimeout(() => document.querySelector<HTMLElement>('#up-next [role=listitem][tabindex="0"]')?.focus({ preventScroll: true }), 700)
}

// ---- M (Shift+F10, the Menu key): the menu of what's selected, its first item selected so the arrows go on in it. A
// button with a menu (⋯) opens it as Enter does; anything else as a right-click there does (a song, a card, a settings
// row). Nothing selected, or nothing with a menu: nothing happens.
/** Right-clicks sent by openMenuOf: the app's own right-click handler (main.ts) leaves them alone. Cancelled there, a
 * menu wouldn't open: it opens a moment after the event, only if nothing cancelled it (which a real right-click allows
 * for, being over before then). */
export const keyRightClicks = new WeakSet<Event>()

export function openMenuOf(el: HTMLElement | null): boolean {
  if (!el || el === document.body) return false
  const button = el.closest<HTMLElement>('[aria-haspopup=menu]')
  if (button) {
    button.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }))
    return true
  }
  const at = el.querySelector<HTMLElement>('[data-song-row]') ?? el
  const box = at.getBoundingClientRect()
  const click = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: box.left + 24, clientY: box.top + box.height / 2 })
  keyRightClicks.add(click)
  at.dispatchEvent(click)
  return true
}

// ---- Y: the lyrics full size, and closed again.
function toggleLyrics() {
  const here = router.currentRoute.value
  if (here.name === 'now-playing' && 'lyrics' in here.query) router.back()
  else if (current.value) router.push({ path: '/now-playing', query: { lyrics: '1' } })
}

function onKey(e: KeyboardEvent) {
  if (e.defaultPrevented || e.altKey || e.metaKey) return
  if ((e.target as HTMLElement | null)?.closest?.('input, textarea, select, [contenteditable]')) return
  const layer = opened()
  if (layer === 'question') return
  const ctrl = e.ctrlKey
  const key = e.key
  const state = nowPlaying.state
  // An action key over a dialog or a menu: closed first, then the action.
  const act = (run: () => unknown) => {
    e.preventDefault()
    if (layer) closeLayer()
    run()
  }

  // Ctrl+↑ / ↓: scroll, smoothly (a dialog's list too).
  if (ctrl && (key === 'ArrowUp' || key === 'ArrowDown')) {
    e.preventDefault()
    const area = scrollable()
    area?.scrollBy({ top: (key === 'ArrowDown' ? 1 : -1) * area.clientHeight * 0.6, behavior: 'smooth' })
    return
  }
  // Ctrl+1–5: the main pages.
  if (ctrl && /^[1-9]$/.test(key)) {
    const page = MAIN_PAGES[Number(key) - 1]
    if (page) act(() => router.push(page.path))
    return
  }
  if (ctrl) {
    if (e.shiftKey && key.toLowerCase() === 'b') act(openReport)
    else if (key === 'ArrowLeft') act(previous)
    else if (key === 'ArrowRight') act(next)
    return
  }
  // The letters and + / −: anywhere (a list or a menu doesn't use them).
  switch (key.length === 1 ? key.toLowerCase() : key) {
    case '+':
    case '=':
    case '-':
      if (!state) return
      return act(() => {
        const up = key !== '-'
        setVolume((state.volume ?? 1) + (up ? volumeStep() : -volumeStep()))
        volumeFlash.at = Date.now()
      })
    case 'l':
      if (current.value) act(() => like(Number(current.value!.id), !current.value!.liked))
      return
    case 's':
      if (state) act(toggleShuffle)
      return
    case 'r':
      if (state) act(cycleRepeat)
      return
    case 'q':
      return act(toggleUpNext)
    case 'y':
      return act(toggleLyrics)
    case '?':
      return act(() => (shortcuts.open = true))
    case 'm':
      // In an open menu, M is its own (it goes to the item starting with M).
      if (layer) return
      if (openMenuOf(document.activeElement as HTMLElement | null)) e.preventDefault()
      return
  }
  if ((key === 'ContextMenu' || (key === 'F10' && e.shiftKey)) && !layer) {
    if (openMenuOf(document.activeElement as HTMLElement | null)) e.preventDefault()
    return
  }
  // Space and the arrows: not where something has them (a list, a slider, a menu, a dialog), nor on a button (Space
  // presses it).
  if (layer || ownsArrows(e.target)) return
  // Full-size lyrics: Space always plays / pauses (the button that opened them, or one of theirs, may have the focus).
  const here = router.currentRoute.value
  const lyricsFull = here.name === 'now-playing' && 'lyrics' in here.query
  const onButton = !lyricsFull && (e.target as HTMLElement | null)?.closest?.('button, a, [role=button], [role=switch], [role=checkbox]')
  if (key === ' ' && !onButton && state) {
    e.preventDefault()
    togglePlay()
  } else if ((key === 'ArrowLeft' || key === 'ArrowRight') && e.shiftKey && state) {
    e.preventDefault()
    seek(Math.max(0, position.value + (key === 'ArrowLeft' ? -1 : 1) * 5))
  }
}

/**
 * Tab within the page: the top bar and the player bar are left out (the mouse has them; the shortcuts do what they
 * do), round from the page's end to its start, scrolling smoothly to what it reaches. Not while a dialog is open (it
 * keeps Tab in itself).
 */
function tabKey(e: KeyboardEvent) {
  if (e.key !== 'Tab' || e.ctrlKey || e.altKey || e.metaKey || opened()) return
  const page = pageArea()
  if (!page) return
  const focusable = [...page.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])')].filter(
    (el) => el.offsetParent && !el.closest('[inert], [aria-hidden=true]') && getComputedStyle(el).visibility !== 'hidden',
  )
  if (!focusable.length) return
  const at = focusable.indexOf(document.activeElement as HTMLElement)
  // Not in the page (the top bar, the player bar, nothing): into it, from its start (or its end, Shift+Tab).
  const to = at < 0 ? (e.shiftKey ? focusable.length - 1 : 0) : (at + (e.shiftKey ? -1 : 1) + focusable.length) % focusable.length
  e.preventDefault()
  focusable[to].focus({ preventScroll: true })
  focusable[to].scrollIntoView({ block: 'nearest', behavior: 'smooth' })
}

/**
 * Keys in a list (role=list: songs, artists, folders): Home / End the first / last, Enter plays a song row (as a
 * double-click does; links and buttons open as they do); M, Shift+F10 or the Menu key opens its menu (openMenuOf). The
 * arrows: arrowNav.ts.
 */
function listKey(e: KeyboardEvent) {
  if (e.defaultPrevented || e.ctrlKey || e.altKey || e.metaKey) return
  const item = (e.target as HTMLElement | null)?.closest?.<HTMLElement>('[role=listitem]')
  const list = item?.closest<HTMLElement>('[role=list]')
  if (!item || !list || (e.target as HTMLElement).closest('input, [role=menu]')) return
  const items = () => [...list.querySelectorAll<HTMLElement>('[role=listitem]')].filter((i) => i.closest('[role=list]') === list)
  switch (e.key) {
    case 'Home':
      e.preventDefault()
      return goTo(items()[0])
    case 'End':
      e.preventDefault()
      return goTo(items().slice(-1)[0])
    case 'Enter':
      // A song row (not a link or a button: those open themselves).
      if ((e.target as HTMLElement).matches('a, button')) return
      e.preventDefault()
      item.querySelector('[data-song-row]')?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))
      return
  }
}

let started = false
/** Once (the computer's frame). */
export function startHotkeys() {
  if (started) return
  started = true
  for (const listener of LISTENERS) window.addEventListener('keydown', listener)
  startArrowNav()
  // A page switch: the focus back at the page's top (the next Tab starts there; nothing on the old page keeps it).
  stopAfterEach = router.afterEach((to, from) => {
    if (to.path === from.path) return
    const active = document.activeElement as HTMLElement | null
    if (active && active !== document.body && !active.closest('[role=dialog]')) active.blur()
  })
}

const LISTENERS = [tabKey, listKey, onKey]
let stopAfterEach: (() => void) | undefined

// Development: a changed file reloads this module (or one it uses); the keys move to the new one. Left alone, the old
// listeners would stay, acting on the old modules (Ctrl+Shift+B opening a report no dialog shows).
if (import.meta.hot) {
  if (import.meta.hot.data.started) startHotkeys()
  import.meta.hot.dispose((data) => {
    data.started = started
    for (const listener of LISTENERS) window.removeEventListener('keydown', listener)
    stopAfterEach?.()
  })
}
