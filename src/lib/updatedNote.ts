/**
 * After an update, it says so once (main.ts: New in Melue, or a note). Apart from lib/whatsNew.ts, so the first screen doesn't load the
 * whole changelog.
 */
const SEEN = 'melue-seen-version'

/**
 * The version Melue was updated from since it last ran (the version it last saw differs), or null. Remembers this one.
 * The first run isn't an update.
 */
export function updatedFrom(version: string): string | null {
  try {
    const seen = localStorage.getItem(SEEN)
    localStorage.setItem(SEEN, version)
    return seen && seen !== version ? seen : null
  } catch {
    return null
  }
}
