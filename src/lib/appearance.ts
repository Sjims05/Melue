/**
 * How some parts look (Display settings, this device's own like the theme): Up next on Now playing and Home's Playing /
 * Continue cards white in dark mode too (as in the concepts), or like the other cards.
 */
import { reactive, watch } from 'vue'

const KEY = 'melue-appearance'
const DEFAULTS = { upNextWhite: true, homeCardsWhite: true }

function saved(): typeof DEFAULTS {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') }
  } catch {
    return { ...DEFAULTS }
  }
}

export const appearance = reactive(saved())
export const APPEARANCE_DEFAULTS = DEFAULTS

watch(appearance, (value) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(value))
  } catch {
    // Not kept without storage.
  }
})
