/**
 * The library as the Library settings show it, shared by the status card and the folder list: the counts, the
 * folders, the songs that can't be found, the songs only on this device (with a share), those all your devices have
 * (offered for the share), the hidden copies of synced songs, and a scan's progress.
 * Read when first shown, and again after anything changes it (a scan, a folder added, songs moved).
 */
import { computed, reactive } from 'vue'
import { library, type LibraryStatus, type MissingSong, type MusicFolder } from '@/library'
import { moving, sync, syncing, type Copies, type LocalSong, type OfferedSong } from '@/library/sync'

export const libraryState = reactive({
  status: null as LibraryStatus | null,
  folders: [] as MusicFolder[],
  /** Songs whose files can't be found. */
  missing: [] as MissingSong[],
  /** Music folders that are the share or look like it (id → why). */
  shareFolders: new Map<number, string>(),
  /** Songs not on the share (on this device; empty without a share), all of them: see `onlyHereSongs`. */
  onlyHere: [] as LocalSong[],
  /** Of those, the ones all your devices have: offered for the share (Songs on all your devices). */
  offered: [] as OfferedSong[],
  copies: null as Copies | null,
  /** A scan running: what it's doing ("Reading tags: 450 of 1.200"), and how far (0 to 1, or null when unknown). */
  scanning: null as { line: string; progress: number | null } | null,
  error: '',
  /** A song list to open when Settings shows (a toast tapped: the songs left out, only on this device). */
  listToOpen: null as 'onlyHere' | null,
})

export async function refreshLibrary(): Promise<void> {
  try {
    const [status, folders, missing, shareFolders] = await Promise.all([
      library.status(),
      library.folders(),
      library.missingSongs(),
      library.shareFolders().catch(() => []),
    ])
    Object.assign(libraryState, { status, folders, missing, shareFolders: new Map(shareFolders) })
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
    await syncing.onToShare((songs) => (libraryState.offered = songs))
    libraryState.offered = await syncing.toShare().catch(() => [])
    await refreshLibrary()
  })()
  return started
}

/** The songs only on this device: not on the share, and not on all your devices either (those are offered apart). */
export const onlyHereSongs = computed(() => {
  const offered = new Set(libraryState.offered.map((s) => s.id))
  return libraryState.onlyHere.filter((s) => !offered.has(s.id))
})

/** The newest scan of any folder (ms), or null before the first. */
export const lastScan = () => Math.max(0, ...libraryState.folders.map((f) => f.lastScanAt ?? 0)) || null
