/**
 * Adding songs from anywhere (desktop): picked with "Add songs…" or dropped on Melue, on any page. They're copied
 * where you choose (DropSongs.vue asks): to the share (and so into the synced music on every device), or into a music
 * folder on this computer, in `Artist/Album/`; with nothing set up, into the Music folder, added as a music folder.
 * The files themselves stay where they are. Dropped on a playlist's page, they can go into that playlist too.
 */
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { reactive } from 'vue'
import { library } from '@/library'
import { describeSyncFailure, moving } from '@/library/sync'
import { refreshLibrary } from './libraryState'

export const MUSIC = ['mp3', 'flac', 'm4a', 'ogg', 'opus', 'wav', 'aac']
const isMusic = (path: string) => MUSIC.includes(path.split('.').pop()?.toLowerCase() ?? '')

/** Where songs go: the share, a music folder here, or the Music folder (added as a music folder first). */
export type AddTo = { kind: 'share' } | { kind: 'folder'; id: number } | { kind: 'music'; path: string }

export const adding = reactive({
  /** The files waiting for the question's answer. */
  files: [] as string[],
  /** Music files being dragged over Melue (0: none). */
  dragging: 0,
  /** Something else takes what's dropped for now (the cover window, for a picture): songs aren't added. */
  paused: false,
  /** What the last add did, or why it didn't work. */
  result: '',
  error: '',
})

export async function pickSongs() {
  const files = await open({ multiple: true, title: 'Add songs', filters: [{ name: 'Music', extensions: MUSIC }] })
  if (files?.length) adding.files = files
}

/** Adds the waiting files (`to`; `playlist`: into it too, once they're in the library). Resolves with what it did. */
export async function addSongs(to: AddTo, playlist: number | null): Promise<string> {
  const files = adding.files
  adding.error = ''
  adding.result = ''
  try {
    if (to.kind === 'share') {
      const r = await moving.moveToSync(files.map((path) => ({ kind: 'file' as const, path })), playlist)
      adding.result = [`${r.moved} added`, r.alreadyThere && `${r.alreadyThere} already on the share`, r.failed.length && `${r.failed.length} failed`]
        .filter(Boolean)
        .join(' · ')
    } else {
      const folder = to.kind === 'folder' ? to.id : (await library.addFolder('path', to.path)).id
      const r = await library.addSongsToFolder(folder, files, playlist)
      adding.result = [`${r.added} added`, r.failed.length && `${r.failed.length} failed (${r.failed[0][0]}: ${r.failed[0][1]})`].filter(Boolean).join(' · ')
    }
  } catch (e) {
    adding.error = to.kind === 'share' ? describeSyncFailure(e) : String(e)
  }
  await refreshLibrary()
  return adding.error || adding.result
}

/** Files dropped on Melue: the music ones wait for the question. Resolves to a function that stops listening. */
export function listenForDrops() {
  return getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (adding.paused) adding.dragging = 0
    else if (payload.type === 'enter') adding.dragging = payload.paths.filter(isMusic).length
    else if (payload.type === 'leave') adding.dragging = 0
    else if (payload.type === 'drop') {
      adding.dragging = 0
      const files = payload.paths.filter(isMusic)
      if (files.length) adding.files = files
    }
  })
}
