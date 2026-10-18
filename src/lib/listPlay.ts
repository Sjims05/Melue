/**
 * How a list's Play and Shuffle buttons play it (an album, a playlist, a folder; Queue settings, this device's own like
 * the theme): with `playFromStart`, Play always starts on its first song with shuffle off, and Shuffle turns shuffle on
 * (from a random song); without, Play leaves shuffle as it is.
 *
 * `songPlays`: a song played from All songs (its play button) plays 'now' (the playing song ends) or 'next' (after it).
 */
import { reactive, watch } from 'vue'

const KEY = 'melue-list-play'
const DEFAULTS = { playFromStart: true, songPlays: 'now' as 'now' | 'next' }

function saved(): typeof DEFAULTS {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') }
  } catch {
    return { ...DEFAULTS }
  }
}

export const listPlay = reactive(saved())
export const LIST_PLAY_DEFAULTS = DEFAULTS

watch(listPlay, (value) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(value))
  } catch {
    // Not kept without storage.
  }
})
