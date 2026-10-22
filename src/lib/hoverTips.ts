/**
 * Hover tips in Melue's style (as the tooltip component: the foreground colour, small text, rounded), for every
 * element with a `title`: the browser's own plain tip is replaced, app-wide, with nothing to change where tips are
 * written. A tip shows after the pointer rests a moment, above the element (below it when there's no room), and goes
 * when the pointer leaves, presses, or the page scrolls. Not for touch (a long press opens menus there). The text stays
 * available to screen readers (as the element's description).
 */

const DELAY_MS = 500
const GAP = 6

let tip: HTMLDivElement | null = null
let showing: HTMLElement | null = null
let timer: ReturnType<typeof setTimeout> | undefined

function tipElement(): HTMLDivElement {
  if (tip) return tip
  tip = document.createElement('div')
  tip.setAttribute('role', 'tooltip')
  tip.className =
    'pointer-events-none fixed z-[100] max-w-72 rounded-md bg-foreground px-3 py-1.5 text-xs text-balance text-background shadow-md transition-opacity duration-150 opacity-0'
  document.body.append(tip)
  return tip
}

/** The element's tip text: its title moved aside (so the browser shows none), kept as its description. */
function textOf(el: HTMLElement): string {
  const title = el.getAttribute('title')
  if (title) {
    el.dataset.tip = title
    el.removeAttribute('title')
    if (!el.hasAttribute('aria-description')) el.setAttribute('aria-description', title)
  }
  return el.dataset.tip ?? ''
}

function show(el: HTMLElement) {
  const text = textOf(el)
  if (!text || !el.isConnected) return
  const t = tipElement()
  t.textContent = text
  const box = el.getBoundingClientRect()
  const size = t.getBoundingClientRect()
  const left = Math.min(Math.max(8, box.left + box.width / 2 - size.width / 2), window.innerWidth - size.width - 8)
  const above = box.top - size.height - GAP
  t.style.left = `${left}px`
  t.style.top = `${above >= 8 ? above : box.bottom + GAP}px`
  t.style.opacity = '1'
  showing = el
}

function hide() {
  clearTimeout(timer)
  showing = null
  if (tip) tip.style.opacity = '0'
}

/** Once, at start. */
export function startHoverTips() {
  document.addEventListener('pointerover', (e) => {
    if (e.pointerType === 'touch') return
    const el = (e.target as Element | null)?.closest?.<HTMLElement>('[title], [data-tip]')
    if (!el || el === showing) return
    hide()
    // The title goes now, so the browser's own tip never shows.
    textOf(el)
    timer = setTimeout(() => show(el), DELAY_MS)
  })
  document.addEventListener('pointerout', (e) => {
    const to = e.relatedTarget as Node | null
    const el = (e.target as Element | null)?.closest?.<HTMLElement>('[data-tip]')
    if (el && (!to || !el.contains(to))) hide()
  })
  document.addEventListener('pointerdown', hide, true)
  document.addEventListener('scroll', hide, true)
  window.addEventListener('blur', hide)
}
