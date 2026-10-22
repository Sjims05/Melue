/**
 * What a bug report says was done just before it (report.ts): the last steps (pages opened, what was clicked or
 * pressed, notes shown) and the errors, kept in memory only. Never what's typed in a field. Names in them (a button
 * says a song's title) are replaced when the report is made (src-tauri report.rs).
 */
import type { RouteLocationNormalized } from 'vue-router'
import { watch } from 'vue'
import { router } from '@/router'
import { shownToast } from '@/lib/toast'

export type Noted = { at: number; what: string }

const STEPS = 20
const ERRORS = 10
const steps: Noted[] = []
const errors: Noted[] = []
/** Paused while the report itself is open (its own clicks aren't steps). */
let paused = false

const keep = (list: Noted[], most: number, what: string) => {
  list.push({ at: Date.now(), what })
  if (list.length > most) list.shift()
}
export const noteStep = (what: string) => !paused && keep(steps, STEPS, what)
export const noteError = (what: string) => keep(errors, ERRORS, what)
export const pauseTrail = (pause: boolean) => (paused = pause)
export const trail = () => ({ steps: [...steps], errors: [...errors] })

/** A page, as a kind ("an album page"): which album isn't the report's business. */
export function pageKind(route: RouteLocationNormalized): string {
  const kinds: Record<string, string> = {
    home: 'Home',
    library: 'Library',
    playlists: 'Playlists',
    'now-playing': 'Now playing',
    settings: 'Settings',
    search: 'the search page',
    artist: 'an artist page',
    album: 'an album page',
    folder: 'a folder page',
    playlist: 'a playlist page',
    'auto-playlist': 'an automatic playlist page',
    dev: 'the dev page',
  }
  let kind = kinds[String(route.name)] ?? 'a page'
  if (route.name === 'now-playing' && 'lyrics' in route.query) kind += ' (lyrics full size)'
  if (route.name === 'library' && route.query.tab) kind += ` (${route.query.tab})`
  return kind
}

/** What was clicked, in words: `“Shuffle”`, "a song (row 3)", "the switch “Sync when Melue starts”". */
export function describe(target: EventTarget | null): string | null {
  const el = (target as Element | null)?.closest?.(
    '[data-song-row], .cover-card, [role=menuitem], [role=menuitemradio], [role=menuitemcheckbox], [role=switch], [role=tab], [role=radio], [role=option], [role=slider], button, a, input, textarea, select',
  ) as HTMLElement | null
  if (!el) return null
  const label = (e: Element | null) => (e?.getAttribute('aria-label') || e?.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 60)
  if (el.matches('[data-song-row]')) {
    const rows = [...(el.closest('[role=list]')?.querySelectorAll('[data-song-row]') ?? [])]
    return `a song${rows.length ? ` (row ${rows.indexOf(el) + 1})` : ''}`
  }
  if (el.matches('.cover-card')) return 'a card'
  if (el.matches('input, textarea, select')) return `the field “${el.getAttribute('aria-label') || el.getAttribute('placeholder') || el.getAttribute('name') || 'text'}”`
  // A switch or a choice says which setting: its row's title.
  const row = el.closest('[data-row]')
  const rowTitle = row ? ` (${label(row.querySelector('[data-row-title]'))})` : ''
  if (el.matches('[role=switch]')) return `the switch${rowTitle}, ${el.getAttribute('aria-checked') === 'true' ? 'was on' : 'was off'}`
  if (el.matches('[role^=menuitem]')) return `“${label(el)}” in a menu`
  if (el.matches('[role=slider]')) return `the slider “${label(el)}”`
  return `“${label(el) || 'a button'}”${rowTitle}`
}

/** A key that isn't typing: "Ctrl+→", "Space", "Escape" (keys in a field aren't kept). */
function keyWords(e: KeyboardEvent): string | null {
  if ((e.target as Element | null)?.closest?.('input, textarea, [contenteditable]')) return null
  if (['Control', 'Shift', 'Alt', 'Meta', 'Tab'].includes(e.key)) return null
  const names: Record<string, string> = { ' ': 'Space', ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→' }
  const key = names[e.key] ?? (e.key.length === 1 ? e.key.toUpperCase() : e.key)
  return [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && (e.key.length > 1 || e.ctrlKey || e.altKey) && 'Shift', key].filter(Boolean).join('+')
}

const errorText = (e: unknown) => (e instanceof Error ? `${e.name}: ${e.message}` : typeof e === 'string' ? e : JSON.stringify(e))

let started = false
/** From the start (main.ts). */
export function startTrail() {
  if (started) return
  started = true
  router.afterEach((to, from) => {
    if (to.fullPath !== from.fullPath) noteStep(`Opened ${pageKind(to)}`)
  })
  document.addEventListener('click', (e) => {
    const what = describe(e.target)
    if (what) noteStep(`Clicked ${what}`)
  }, true)
  document.addEventListener('contextmenu', (e) => {
    const what = describe(e.target)
    noteStep(`Opened the menu of ${what ?? 'the page'}`)
  }, true)
  document.addEventListener('keydown', (e) => {
    const key = keyWords(e)
    if (key) noteStep(`Pressed ${key}`)
  }, true)
  watch(shownToast, (shown) => shown && noteStep(`A note showed: “${shown.text}”`))
  window.addEventListener('error', (e) => noteError(errorText(e.error ?? e.message)))
  window.addEventListener('unhandledrejection', (e) => noteError(errorText(e.reason)))
  // What Melue itself reports as an error (console.error("melue: …", e)).
  const consoleError = console.error.bind(console)
  console.error = (...args: unknown[]) => {
    noteError(args.map(errorText).join(' '))
    consoleError(...args)
  }
}
