/**
 * The library as the Library settings show it, shared by the status card and the folder list: the counts, the
 * folders, the songs only on this device (with a share), the hidden copies of synced songs, and a scan's progress.
 * Read when first shown, and again after anything changes it (a scan, a folder added, songs moved).
 */
import { reactive } from 'vue'
import { library, type LibraryStatus, type MusicFolder } from '@/library'
import { moving, sync, type Copies, type LocalSong } from '@/library/sync'

export const libraryState = reactive({
  status: null as LibraryStatus | null,
  folders: [] as MusicFolder[],
  /** Songs only on this device (not on the share); empty without a share. */
  onlyHere: [] as LocalSong[],
  copies: null as Copies | null,
  /** A scan running: what it's doing ("Reading tags: 450 of 1.200"), and how far (0 to 1, or null when unknown). */
  scanning: null as { line: string; progress: number | null } | null,
  error: '',
})

export async function refreshLibrary(): Promise<void> {
  try {
    const [status, folders] = await Promise.all([library.status(), library.folders()])
    Object.assign(libraryState, { status, folders })
    const [onlyHere, copies] = sync.available
      ? await Promise.all([moving.notOnShare(), moving.copies()])
      : [[], null]
    Object.assign(libraryState, { onlyHere, copies, error: '' })
  } catch (e) {
    libraryState.error = `Can’t read the library: ${e}`
  }
}

let started: Promise<void> | null = null

/** Once: reads it, then follows scans and changes. */
export function startLibraryState(): Promise<void> {
  started ??= (async () => {
    await library.onScan((event) => {
      if (event.phase === 'started') libraryState.scanning = { line: 'Looking for songs…', progress: null }
      else if (event.phase === 'folder') libraryState.scanning = { line: `Looked through ${event.name}`, progress: null }
      else if (event.phase === 'tags') {
        libraryState.scanning = { line: `Reading tags: ${event.done.toLocaleString()} of ${event.total.toLocaleString()}`, progress: event.total ? event.done / event.total : null }
      } else {
        libraryState.scanning = null
        refreshLibrary()
      }
    })
    await library.onChange((change) => {
      if (change.reason !== 'likes') refreshLibrary()
    })
    await moving.onCopies((copies) => (libraryState.copies = copies))
    await refreshLibrary()
  })()
  return started
}

/** The newest scan of any folder (ms), or null before the first. */
export const lastScan = () => Math.max(0, ...libraryState.folders.map((f) => f.lastScanAt ?? 0)) || null
