/**
 * A page following the library's changes: `wanted` says which ones concern it (a page reloads only for what it shows:
 * a playlist edit doesn't reload all songs), and `reload` runs then: at once while the page is shown; while it's put
 * aside (pages stay alive once visited), once when it's shown again, however many changes came meanwhile.
 */
import { onActivated, onDeactivated, onUnmounted } from 'vue'
import { library, type LibraryChange } from '@/library'

export function onLibraryChange(wanted: (change: LibraryChange) => boolean, reload: () => unknown) {
  let shown = true
  let stale = false
  onActivated(() => {
    shown = true
    if (!stale) return
    stale = false
    reload()
  })
  onDeactivated(() => (shown = false))
  const stop = library.onChange((change) => {
    if (!wanted(change)) return
    if (shown) reload()
    else stale = true
  })
  onUnmounted(() => void stop.then((unlisten) => unlisten()))
}

/** What a list of the whole library (songs, albums, artists, folders) shows: not likes, not playlists. */
export const changesTheLibrary = (change: LibraryChange) => change.reason !== 'likes' && change.reason !== 'playlists'
