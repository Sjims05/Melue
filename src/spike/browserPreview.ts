/**
 * Previewing the frontend in a plain browser (`npm run dev`, then http://localhost:1420/?page=components): there's no
 * Melue behind it there, so its calls get sample answers instead (Tauri's own mockIPC). Only loaded by main.ts in a
 * dev browser, never in the app or its build.
 *
 * `?status=` picks the share's state: synced (default), syncing, error, password, none, streamed.
 */
import { emit } from '@tauri-apps/api/event'
import { mockIPC } from '@tauri-apps/api/mocks'
import type { LayoutStatus, PlaybackSettings, ShareInfo, SyncResult, SyncSettings, SyncStatus } from '@/library/sync'

const minutes = (n: number) => n * 60_000
const status = new URLSearchParams(location.search).get('status') ?? 'synced'

const result = (copied: number, moved: number): SyncResult => ({
  copied, bytes: copied * 9_000_000, moved, adopted: 0, removed: 0, failed: [], waitingForWifi: 0, removalsWaiting: 0, cancelled: false, stopped: null,
})

let info: ShareInfo = {
  settings: status === 'none' ? null
    : status === 'streamed' ? { kind: 'drive', path: 'Z:\\Music', stream: true, syncFolderId: 3 }
    : { kind: 'smb', host: 'nas', share: 'music', folder: '', user: 'simon', passwordStore: 'secure', syncFolderId: 3 },
  syncFolder: null,
  needsPassword: status === 'password',
  defaultSyncFolder: 'C:\\Users\\simon\\Music\\Melue',
}

let syncStatus: SyncStatus = {
  syncing: status === 'syncing' ? { filesDone: 3, filesTotal: 12, bytesDone: 45_000_000, bytesTotal: 120_000_000, current: 'Northern Arc/Orbit.mp3' } : null,
  last: { atMs: Date.now() - (status === 'error' ? minutes(60 * 20) : minutes(12)), trigger: 'return', result: result(3, 1) },
  problem: status === 'error' ? { kind: 'setup', error: { kind: 'share', error: { kind: 'unreachable', detail: 'timed out' } } } : null,
  retryAtMs: status === 'error' ? Date.now() + minutes(15) : null,
  note: null,
}

let settings: SyncSettings = {
  removeGone: true, askBeforeRemoving: 5, atStart: false, onReturnAfterMinutes: 15, onChange: true, everyMinutes: 30,
  copyOnMetered: false, moveNewSongs: false, shareLayout: 'artistAlbum',
}
// The share's songs are in Artist/Album; picking One folder leaves 3 to move.
const layoutStatus = (): LayoutStatus => settings.shareLayout === 'artistAlbum'
  ? { layout: 'artistAlbum', applied: true, toMove: 0, examples: [] }
  : { layout: 'oneFolder', applied: false, toMove: 3, examples: [{ from: 'ABBA/Waterloo/Waterloo.mp3', to: 'Waterloo.mp3' }] }
let reorganised = false

let playback: PlaybackSettings = { mode: 'ask', deviceName: 'TROLLGANG', continueWithinMinutes: 10 }

const wait = (ms: number) => new Promise((done) => setTimeout(done, ms))

// A pretend sync: progress for a few seconds, then the result.
async function pretendSync() {
  for (let done = 0; done <= 4; done++) {
    syncStatus = { ...syncStatus, problem: null, retryAtMs: null, syncing: { filesDone: done, filesTotal: 4, bytesDone: done * 9_000_000, bytesTotal: 36_000_000, current: done < 4 ? `ABBA/Arrival/Song ${done + 1}.mp3` : null } }
    await emit('sync-status', syncStatus)
    await wait(700)
  }
  syncStatus = { ...syncStatus, syncing: null, last: { atMs: Date.now(), trigger: 'button', result: result(4, 0) } }
  await emit('sync-status', syncStatus)
  return syncStatus.last!.result
}

export function installBrowserPreview() {
  mockIPC(
    async (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>
      switch (cmd) {
        case 'library_sync_available': return info.settings !== null
        case 'sync_status': return syncStatus
        case 'share_info': return info
        case 'share_set_password': info = { ...info, needsPassword: false }; return info
        case 'share_set_home_host':
          if (info.settings?.kind === 'smb') info = { ...info, settings: { ...info.settings, homeHost: (a.homeHost as string) ?? undefined } }
          return info
        case 'sync_now': return pretendSync()
        case 'sync_cancel': return null
        case 'sync_settings': return settings
        case 'sync_set_settings': settings = a.settings as SyncSettings; reorganised = false; return settings
        case 'sync_layout_status': return reorganised ? { ...layoutStatus(), applied: true, toMove: 0 } : layoutStatus()
        case 'sync_reorganise': await wait(1200); reorganised = true; return { moved: 3, playlistsUpdated: 2, failed: [] }
        case 'playback_sync_settings': return playback
        case 'playback_sync_set_settings': playback = a.settings as PlaybackSettings; return playback
        default: return null
      }
    },
    { shouldMockEvents: true },
  )
  console.info('melue: browser preview, with sample data (status:', status + ')')
}
