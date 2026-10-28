/**
 * Dev: trying the light theme's page and card colours (the Dev page's pickers), and borders around its cards. Applied over style.css's light colours,
 * in the light theme and in light areas (Up next and Home's white cards in dark mode); not saved: a restart goes back
 * to style.css.
 */
import { reactive, watch } from 'vue'

/** The ones tried (2026-10-28), and style.css's now. */
export const COLOR_PRESETS = [
  ['now', 'Now', '#f0f0f0', '#ffffff'],
  ['f3', '#f3f3f3', '#f3f3f3', '#ffffff'],
  ['old', 'Old', '#ededed', '#ffffff'],
  ['swapped', 'Swapped', '#ffffff', '#ededed'],
  ['f5', '#f5f5f5', '#f5f5f5', '#ffffff'],
] as const

export const devColors = reactive({ page: '#f0f0f0', card: '#ffffff', on: false, borders: false, lines: 'page' as 'page' | 'border' })

let style: HTMLStyleElement | null = null
watch(
  devColors,
  ({ page, card, on, borders, lines }) => {
    if (!on && !borders) return void (style?.remove(), (style = null))
    if (!style) {
      style = document.createElement('style')
      style.id = 'dev-colors'
    }
    // Last in <head>: after style.css, so it wins over `.light-area`'s own colours too.
    document.head.appendChild(style)
    style.textContent =
      (on ? `:root:not(.dark), .light-area { --background: ${page}; --card: ${card}; }` : '') +
      // Cards (rounded, in the card colour: Settings, Home, Up next, lyrics, dialogs, the phone's menu drawer) outlined
      // inside their edge: nothing moves. Not the ones that have a border already.
      (borders ? `:root:not(.dark) .bg-card:is(.rounded-xl, .rounded-2xl, .rounded-t-2xl):not(.border) { outline: 1px solid var(--border); outline-offset: -1px; }` : '') +
      // The lines between settings in the border's colour, as thin as the border.
      (borders && lines === 'border' ? `:root:not(.dark) { --separator-color: var(--border); --separator: 1px; }` : '')
  },
  { deep: true },
)

export function applyColorPreset(key: (typeof COLOR_PRESETS)[number][0]) {
  const preset = COLOR_PRESETS.find((p) => p[0] === key)!
  Object.assign(devColors, { page: preset[2], card: preset[3], on: true })
}

export function resetDevColors() {
  Object.assign(devColors, { page: '#f0f0f0', card: '#ffffff', on: false, borders: false, lines: 'page' })
}
