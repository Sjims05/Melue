/**
 * Light or dark: the `dark` class on <html> (the theme's colours are in style.css). Follows the system unless set;
 * `adaptive` switches by the clock: dark from one time, light from another (e.g. dark 21:00, light 07:00).
 *
 * For now the choice is kept in this device's local storage; the Theme setting (synced across devices) takes over
 * once settings sync.
 */
import { reactive, ref, watch } from 'vue'

export type ThemeMode = 'system' | 'dark' | 'light' | 'adaptive'
const MODES: ThemeMode[] = ['system', 'dark', 'light', 'adaptive']

const KEY = 'melue-theme'
const TIMES_KEY = 'melue-theme-times'
/** Adaptive's times, "HH:MM" (24 hours). */
export const THEME_TIMES_DEFAULT = { dark: '21:00', light: '07:00' }

const system = window.matchMedia('(prefers-color-scheme: dark)')

function read<T>(key: string, fallback: T, parse: (value: string) => T | null): T {
  try {
    const value = localStorage.getItem(key)
    return (value !== null && parse(value)) || fallback
  } catch {
    return fallback
  }
}
function keep(key: string, value: string) {
  try {
    localStorage.setItem(key, value)
  } catch {
    // Not kept: it still applies until Melue closes.
  }
}

export const themeMode = ref<ThemeMode>(read<ThemeMode>(KEY, 'system', (v) => (MODES.includes(v as ThemeMode) ? (v as ThemeMode) : null)))
export const themeTimes = reactive(read(TIMES_KEY, { ...THEME_TIMES_DEFAULT }, (v) => ({ ...THEME_TIMES_DEFAULT, ...JSON.parse(v) })))

const minutes = (time: string) => {
  const [h, m] = time.split(':').map(Number)
  return h * 60 + m
}

/** Adaptive: whether it's dark now (dark from `dark` until `light`, across midnight when needed). */
export function darkByClock(now = new Date(), times = themeTimes): boolean {
  const t = now.getHours() * 60 + now.getMinutes()
  const [dark, light] = [minutes(times.dark), minutes(times.light)]
  if (dark === light) return false
  return dark < light ? t >= dark && t < light : t >= dark || t < light
}

function apply() {
  const mode = themeMode.value
  const dark = mode === 'dark' || (mode === 'system' && system.matches) || (mode === 'adaptive' && darkByClock())
  document.documentElement.classList.toggle('dark', dark)
}

watch(themeMode, (mode) => {
  keep(KEY, mode)
  apply()
})
watch(themeTimes, (times) => {
  keep(TIMES_KEY, JSON.stringify(times))
  apply()
})
system.addEventListener('change', apply)
// Adaptive: the clock is checked every half minute (and when Melue comes back to the front).
setInterval(() => themeMode.value === 'adaptive' && apply(), 30_000)
document.addEventListener('visibilitychange', apply)
apply()
