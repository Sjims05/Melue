/**
 * How some parts look (Display settings, this device's own like the theme): Up next on Now playing and Home's Playing /
 * Continue cards in the primary colour (white in dark mode, black in light mode: they stand out in both, as in the
 * concepts), or like the other cards; the page Melue opens on (Start on: Home, Library, or where it was left); and on
 * a phone, whether the mini player hides while nothing is loaded (it stays by default, empty, like a computer's bar).
 */
import { computed, reactive, watch } from 'vue'
import { isDark } from './theme'

const KEY = 'melue-appearance'
export type StartOn = 'home' | 'library' | 'last'
const DEFAULTS = { upNextPrimary: true, homeCardsPrimary: true, startOn: 'home' as StartOn, hideEmptyMiniPlayer: false }

function saved(): typeof DEFAULTS {
  try {
    const kept = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    // Kept before as "white" (the same choice: white was the primary colour in dark mode).
    return { ...DEFAULTS, upNextPrimary: kept.upNextWhite ?? DEFAULTS.upNextPrimary, homeCardsPrimary: kept.homeCardsWhite ?? DEFAULTS.homeCardsPrimary, ...kept }
  } catch {
    return { ...DEFAULTS }
  }
}

export const appearance = reactive(saved())
export const APPEARANCE_DEFAULTS = DEFAULTS

/** A panel in the primary colour: the other theme's colours inside it (a light area in dark mode, a dark one in light). */
export const primaryArea = computed(() => (isDark.value ? 'light-area' : 'dark-area'))

watch(appearance, (value) => {
  try {
    localStorage.setItem(
      KEY,
      JSON.stringify({ upNextPrimary: value.upNextPrimary, homeCardsPrimary: value.homeCardsPrimary, startOn: value.startOn, hideEmptyMiniPlayer: value.hideEmptyMiniPlayer }),
    )
  } catch {
    // Not kept without storage.
  }
})
