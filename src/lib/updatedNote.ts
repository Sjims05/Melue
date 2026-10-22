/**
 * After an update, a note says so once (main.ts). Apart from lib/whatsNew.ts, so the first screen doesn't load the
 * whole changelog.
 */
const SEEN = 'melue-seen-version'

/**
 * Whether Melue was updated since it last ran (the version it last saw differs). Remembers this one. The first run
 * isn't an update.
 */
export function updatedSinceLastRun(version: string): boolean {
  try {
    const seen = localStorage.getItem(SEEN)
    localStorage.setItem(SEEN, version)
    return !!seen && seen !== version
  } catch {
    return false
  }
}
