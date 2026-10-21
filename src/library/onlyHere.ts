/**
 * The songs only on this device (with a share set up: in a music folder that isn't synced, and not on the share), for
 * their "This device" badge and Move to sync in their menu, everywhere they're shown. Read again when a share is set
 * up or removed, and after the library changes (a scan, a sync, songs moved), a moment after the last change.
 */
import { shallowRef, watch } from 'vue'
import { library } from '.'
import { moving, sync } from './sync'

export const onlyHere = shallowRef<ReadonlySet<number>>(new Set())
/** Of those, the ones in a network folder (moving one deletes its original for good: no Recycle Bin). */
export const onNetwork = shallowRef<ReadonlySet<number>>(new Set())

let started = false
let waiting: ReturnType<typeof setTimeout> | undefined

/** Once (the app's frame): keeps `onlyHere` up to date. */
export function followOnlyHere() {
  if (started) return
  started = true
  watch(() => sync.available, read, { immediate: true })
  library.onChange((change) => {
    if (change.reason === 'likes') return
    clearTimeout(waiting)
    waiting = setTimeout(read, 1000)
  })
}

async function read() {
  if (!sync.available) {
    onlyHere.value = new Set()
    onNetwork.value = new Set()
    return
  }
  const songs = await moving.notOnShare().catch(() => null)
  if (!songs) return
  onlyHere.value = new Set(songs.map((s) => s.id))
  onNetwork.value = new Set(songs.filter((s) => s.network).map((s) => s.id))
}
