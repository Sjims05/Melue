/**
 * The screen in words, for a bug report instead of a screenshot (which would show the person's music): the page as a
 * kind, its title and sections, lists and cards by count, tips shown, what has the focus, a dialog or menu open, and
 * the player. Names in it are replaced when the report is made (src-tauri report.rs).
 */
import { router } from '@/router'
import { isMobile } from '@/platform'
import { nowPlaying, position } from '@/player/nowPlaying'
import { describe, pageKind } from './trail'

const text = (el: Element | null | undefined) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 80)
const label = (el: Element) => (el.getAttribute('aria-label') || text(el)).slice(0, 40)
const visible = (el: Element) => !!(el as HTMLElement).offsetParent && !el.closest('[inert], [aria-hidden=true]')
const time = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

/** The page in view: the page opened on top, else the main page (as hotkeys.ts finds it). */
function pageArea(): HTMLElement | null {
  const all = [...document.querySelectorAll<HTMLElement>('[data-scroller]')].filter((el) => el.offsetParent && !el.closest('[inert]'))
  return all[all.length - 1] ?? null
}

export function outline(): string {
  const lines: string[] = []
  const route = router.currentRoute.value
  const dark = document.documentElement.classList.contains('dark')
  lines.push(`Page: ${pageKind(route)} (${isMobile ? 'phone' : `window ${innerWidth}×${innerHeight}`}, ${dark ? 'dark' : 'light'})`)

  const page = pageArea()
  if (page) {
    const title = page.querySelector('h1')
    if (title) lines.push(`Title: “${text(title)}”`)
    const sections = [...page.querySelectorAll('h2, h3')].filter(visible).map(text).filter(Boolean)
    if (sections.length) lines.push(`Sections: ${sections.slice(0, 14).join(' · ')}${sections.length > 14 ? ` (and ${sections.length - 14} more)` : ''}`)
    for (const list of page.querySelectorAll('[role=list]')) {
      if (!visible(list)) continue
      const rows = list.querySelectorAll('[data-song-row]').length
      const total = Number(list.getAttribute('aria-rowcount') || list.getAttribute('data-count')) || rows
      if (rows) lines.push(`A list of ${total} song${total === 1 ? '' : 's'}${total > rows ? ` (${rows} drawn)` : ''}`)
    }
    for (const grid of page.querySelectorAll('.cover-grid')) {
      if (visible(grid)) lines.push(`Cards: ${grid.querySelectorAll('.cover-card').length}`)
    }
    const tips = [...page.querySelectorAll('[data-copy]')].filter(visible).map(text)
    for (const tip of tips.slice(0, 5)) lines.push(`Error shown: “${tip}”`)
    const scrolled = page.scrollHeight > page.clientHeight ? Math.round((page.scrollTop / (page.scrollHeight - page.clientHeight)) * 100) : null
    if (scrolled != null) lines.push(`Scrolled: ${scrolled}% down`)
  }

  const focused = document.activeElement && document.activeElement !== document.body ? describe(document.activeElement) : null
  if (focused) lines.push(`Focus: ${focused}`)

  for (const layer of document.querySelectorAll('[role=dialog], [role=alertdialog]')) {
    const title = layer.querySelector('h2, [id$=title]')
    const buttons = [...layer.querySelectorAll('button')].filter(visible).map(label).filter(Boolean)
    lines.push(`Open: ${layer.getAttribute('role') === 'alertdialog' ? 'a question' : 'a dialog'} “${text(title)}”${buttons.length ? `, buttons: ${buttons.slice(0, 8).join(', ')}` : ''}`)
  }
  for (const menu of document.querySelectorAll('[role=menu], [role=listbox]')) {
    const items = [...menu.querySelectorAll('[role^=menuitem], [role=option]')].map(label).filter(Boolean)
    lines.push(`Open: a menu: ${items.slice(0, 12).join(', ')}`)
  }

  const state = nowPlaying.state
  if (state && state.duration) {
    lines.push(
      `Player: ${state.isPlaying ? 'playing' : 'paused'} at ${time(position.value)} of ${time(state.duration)}` +
        ` · shuffle ${state.shuffle ? 'on' : 'off'} · repeat ${state.repeatMode} · volume ${Math.round((state.volume ?? 1) * 100)}%`,
    )
  } else lines.push('Player: nothing playing')
  return lines.join('\n')
}
