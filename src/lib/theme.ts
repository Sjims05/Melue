/**
 * Light or dark: the `dark` class on <html> (the theme's colours are in style.css). Follows the system unless set.
 *
 * For now the choice is kept in this device's local storage; the Theme setting (synced across devices) takes over
 * once settings sync.
 */
import { ref, watch } from 'vue'

export type ThemeMode = 'system' | 'dark' | 'light'

const KEY = 'melue-theme'
const system = window.matchMedia('(prefers-color-scheme: dark)')

function saved(): ThemeMode {
  try {
    const value = localStorage.getItem(KEY)
    return value === 'dark' || value === 'light' ? value : 'system'
  } catch {
    return 'system'
  }
}

export const themeMode = ref<ThemeMode>(saved())

function apply() {
  const dark = themeMode.value === 'dark' || (themeMode.value === 'system' && system.matches)
  document.documentElement.classList.toggle('dark', dark)
}

watch(themeMode, (mode) => {
  try {
    localStorage.setItem(KEY, mode)
  } catch {
    // Not kept: it still applies until Melue closes.
  }
  apply()
})
system.addEventListener('change', apply)
apply()
