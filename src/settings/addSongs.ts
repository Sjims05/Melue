/**
 * Adding songs from anywhere (desktop, with a share): picked with "Add songs…" or dropped on Melue. They're copied
 * to the share (and so into the synced music on every device); the files stay where they are. Both ways end in the
 * same question (DropSongs.vue asks it).
 */
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { reactive } from 'vue'
import { describeSyncFailure, moving } from '@/library/sync'
import { refreshLibrary } from './libraryState'

export const MUSIC = ['mp3', 'flac', 'm4a', 'ogg', 'opus', 'wav', 'aac']
const isMusic = (path: string) => MUSIC.includes(path.split('.').pop()?.toLowerCase() ?? '')

export const adding = reactive({
  /** The files waiting for the question's answer. */
  files: [] as string[],
  /** Music files being dragged over Melue (0: none). */
  dragging: 0,
  /** What the last add did, or why it didn't work. */
  result: '',
  error: '',
})

export async function pickSongs() {
  const files = await open({ multiple: true, title: 'Add songs', filters: [{ name: 'Music', extensions: MUSIC }] })
  if (files?.length) adding.files = files
}

export async function addSongs() {
  const files = adding.files
  adding.files = []
  adding.error = ''
  try {
    const r = await moving.moveToSync(files.map((path) => ({ kind: 'file' as const, path })))
    const parts = [`${r.moved} added`, r.alreadyThere && `${r.alreadyThere} already on the share`, r.failed.length && `${r.failed.length} failed`]
    adding.result = parts.filter(Boolean).join(' · ')
  } catch (e) {
    adding.error = describeSyncFailure(e)
  }
  await refreshLibrary()
}

/** Files dropped on Melue: the music ones wait for the question. Resolves to a function that stops listening. */
export function listenForDrops() {
  return getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (payload.type === 'enter') adding.dragging = payload.paths.filter(isMusic).length
    else if (payload.type === 'leave') adding.dragging = 0
    else if (payload.type === 'drop') {
      adding.dragging = 0
      const files = payload.paths.filter(isMusic)
      if (files.length) adding.files = files
    }
  })
}
