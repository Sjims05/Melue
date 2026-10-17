/**
 * Previewing the frontend in a plain browser (`npm run dev`, then http://localhost:1420/?page=components): there's no
 * Melue behind it there, so its calls get sample answers instead (Tauri's own mockIPC). Only loaded by main.ts in a
 * dev browser, never in the app or its build.
 *
 * `?status=` picks the share's state: synced (default), syncing, error, password, none, streamed. `?output=unplugged`:
 * the chosen output device (headphones) isn't connected. `?library=empty`: no music folders yet. `?player=empty`:
 * nothing in the queue (otherwise eight songs from Liked songs, the first playing 54 s in).
 */
import { emit } from '@tauri-apps/api/event'
import { mockIPC } from '@tauri-apps/api/mocks'
import type { AutoPlaylist, LibraryStatus, MusicFolder, PlayerSettings, QueueFollow } from '@/library'
import type { Copies, DeletedPlaylist, LayoutStatus, LocalSong, PlaybackSettings, ShareInfo, ShareSetup, SyncResult, SyncSettings, SyncStatus } from '@/library/sync'
import type { OutputDevices } from '@/platform'
import { PLAYER_DEFAULTS, QUEUE_FOLLOW_DEFAULTS } from '@/settings/defaults'

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

// The player: eight songs from Liked songs, the first playing (its state only changes through the controls here).
const QUEUE_SONGS: [string, string, string, number][] = [
  ['Golden hour', 'Amara Sol', 'Soft focus', 252], ['Slow motion', 'Elias Bloom', 'After the rain', 226],
  ['Stay a little longer', 'June & the Satellites', 'In your orbit', 212], ['In your orbit', 'Theo Banks', 'In your orbit', 212],
  ['Sunday kind of love', 'Nia Rivers', 'Sunday kind of love', 265], ['Coastline', 'Oliver Hayes', 'Open water', 198],
  ['All the way home', 'Mira Okafor', 'The long way', 221], ['Midnight City Lights', 'Neon Dreams', 'Someway', 225],
]
const noQueue = new URLSearchParams(location.search).get('player') === 'empty'
let queueItems = noQueue ? [] : QUEUE_SONGS.map(([title, artist, album], i) => ({ id: 201 + i, src: `C:/Music/${title}.mp3`, title, artist, album }))
let playerState = {
  status: 'playing', currentTime: 54, duration: 252, isPlaying: true, queueIndex: noQueue ? -1 : 0, queueLength: queueItems.length,
  currentId: queueItems[0]?.id, shuffle: false, repeatMode: 'off', sleepTimerEndsAtMs: null, sleepTimerEndOfTrack: false, volume: 0.7,
}
const playerQueue = () => ({
  items: queueItems,
  currentIndex: playerState.queueIndex,
  playOrder: queueItems.map((_, i) => i),
  // A made-up next pass (the list backwards from the first one), to see the preview's own order.
  nextPassOrder: playerState.shuffle && playerState.repeatMode === 'all' ? [0, ...queueItems.map((_, i) => i).slice(1).reverse()] : null,
  sourceId: 'liked',
})
const atIndex = (i: number) => {
  playerState = { ...playerState, queueIndex: i, currentId: queueItems[i]?.id, currentTime: 0, duration: QUEUE_SONGS.find((q) => q[0] === queueItems[i]?.title)?.[3] ?? 200 }
  return playerState
}
const queueTracks = (ids: number[]) =>
  queueItems.filter((i) => ids.includes(i.id)).map((i) => ({
    id: i.id, folderId: 1, title: i.title, artist: i.artist, album: i.album, albumArtist: i.artist, trackNo: null, discNo: null, year: 2023,
    genre: null, durationMs: (QUEUE_SONGS.find((q) => q[0] === i.title)?.[3] ?? 200) * 1000, format: 'mp3', bitrateKbps: 320, sampleRate: 44100,
    cover: null, tagsRead: true, tagsError: null, available: true, liked: i.id === 202, playCount: 3,
  }))

// Playlists: the auto ones (one hidden), and two deleted lately.
let autoPlaylists: AutoPlaylist[] = [
  { source: 'liked', name: 'Liked songs', kind: 'liked', songs: 86, size: null, hidden: false },
  { source: 'top:month', name: 'Top songs this month', kind: 'top', songs: 50, size: 50, hidden: true },
  { source: 'top:year', name: 'Top songs this year', kind: 'top', songs: 50, size: 50, hidden: false },
] as AutoPlaylist[]
let deleted: DeletedPlaylist[] = [
  { syncId: 'gym', name: 'Gym', songs: 12, deletedAt: Date.now() - minutes(14 * 24 * 60) },
  { syncId: 'road', name: 'Road trip', songs: 31, deletedAt: Date.now() - minutes(19 * 24 * 60) },
]

// Melue's own settings (library_setting), and starting with the system.
const kept = new Map<string, string>()
let autostart = true
let coversBytes = 84_000_000

let queueFollow: QueueFollow = { ...QUEUE_FOLLOW_DEFAULTS }
let player: PlayerSettings = { ...PLAYER_DEFAULTS }
// Headphones that were chosen (the names are remembered from when they were there).
const unplugged = new URLSearchParams(location.search).get('output') === 'unplugged'
if (unplugged) localStorage.setItem('melue-output-names', JSON.stringify({ arctis: 'Headphones (Arctis 7)' }))
let outputs: OutputDevices = {
  devices: [
    { id: 'sonar', name: 'SteelSeries Sonar - Media', isDefault: false },
    { id: 'realtek', name: 'Speakers (Realtek)', isDefault: true },
    { id: 'nvidia', name: 'LC24RG50 (Nvidia High Definition Audio)', isDefault: false },
  ],
  selected: unplugged ? 'arctis' : 'sonar',
  active: unplugged ? 'realtek' : 'sonar',
}

// The library: two folders of your own and the synced one; Music_Test has songs only here, and there are copies.
const day = (n: number) => Date.now() - n * 86_400_000
const folder = (id: number, name: string, location: string, tracks: number, synced = false): MusicFolder => ({
  id, kind: 'path', location, name, enabled: true, synced, addedAt: day(30), lastScanAt: Date.now() - minutes(42), tracks,
} as MusicFolder)
let folders: MusicFolder[] = new URLSearchParams(location.search).get('library') === 'empty' ? [] : [
  folder(1, 'Music', 'C:\Users\simon\Music', 1012),
  folder(2, 'Music_Test', 'D:\Music_Test', 4),
  folder(3, 'Synced music', 'C:\Users\simon\Music\Melue', 218, true),
]
let onlyHere: LocalSong[] = [
  { id: 101, title: 'The Sign', artist: 'Ace of Base', album: 'Happy Nation', folder: 'Music_Test', size: 7_400_000 },
  { id: 102, title: 'Waterloo', artist: 'ABBA', album: 'Waterloo', folder: 'Music_Test', size: 6_100_000 },
  { id: 103, title: 'Dancing Queen', artist: 'ABBA', album: 'Arrival', folder: 'Music', size: 8_900_000 },
]
let copies: Copies = { songs: 12, bytes: 96_000_000 }
const libraryStatus = (): LibraryStatus => ({
  schemaVersion: 1, folders: folders.length, tracks: folders.filter((f) => f.enabled).reduce((n, f) => n + f.tracks, 0),
  albums: folders.length ? 98 : 0, artists: folders.length ? 64 : 0, playlists: folders.length ? 12 : 0,
})
const changed = () => emit('library-changed', { reason: 'folders', tracks: [], sources: [] })

const wait = (ms: number) => new Promise((done) => setTimeout(done, ms))

// A pretend scan: its phases, with the tags read in steps.
async function pretendScan() {
  await emit('library-scan', { phase: 'started' })
  await wait(600)
  for (let done = 0; done <= 1200; done += 300) {
    await emit('library-scan', { phase: 'tags', done, total: 1200 })
    await wait(500)
  }
  folders = folders.map((f) => ({ ...f, lastScanAt: Date.now() }))
  await emit('library-scan', { phase: 'finished', summary: {}, changed: false })
  return null
}

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
        case 'plugin:app|version': return '0.5.1'
        case 'share_set_up': {
          const setup = a.setup as ShareSetup
          await wait(900)
          // "wrong" as the password shows the error under the password field; host "nowhere" can't be reached.
          if (setup.kind === 'smb' && setup.host === 'nowhere') throw { kind: 'share', error: { kind: 'unreachable', detail: 'timed out' } }
          if (a.password === 'wrong') throw { kind: 'share', error: { kind: 'login', detail: 'denied' } }
          info = { ...info, settings: { ...setup, syncFolderId: 3 }, syncFolder: folders.find((f) => f.synced) ?? null }
          return info
        }
        case 'share_remove': info = { ...info, settings: null }; return info
        case 'share_set_password': info = { ...info, needsPassword: false }; return info
        case 'share_set_shown_in_other_apps': return null
        case 'library_auto_playlists': return autoPlaylists
        case 'library_set_auto_playlist': {
          autoPlaylists = autoPlaylists.map((p) => (p.source === a.source ? { ...p, ...(a.hidden !== undefined && { hidden: a.hidden as boolean }), ...(a.size !== undefined && { size: a.size as number, songs: a.size as number }) } : p))
          return autoPlaylists
        }
        case 'deleted_playlists': return deleted
        case 'restore_playlist': deleted = deleted.filter((p) => p.syncId !== a.syncId); return 1
        case 'library_music_access': return { permission: 'prompt', songs: null }
        case 'library_folder_covers': return { enabled: false, permission: 'granted' }
        case 'library_setting': return kept.get(a.key as string) ?? null
        case 'library_set_setting': kept.set(a.key as string, a.value as string); return null
        case 'plugin:autostart|is_enabled': return autostart
        case 'plugin:autostart|enable': autostart = true; return null
        case 'plugin:autostart|disable': autostart = false; return null
        case 'library_covers_size': return coversBytes
        case 'library_clean_covers': await wait(600); coversBytes -= 6_000_000; return 8
        case 'library_status': return libraryStatus()
        case 'library_folders': return folders
        case 'library_scan': return pretendScan()
        case 'library_add_folder': {
          const added = folder(Date.now(), String(a.location).split(/[\/]/).pop() || 'Folder', String(a.location), 0)
          folders = [...folders, added]
          await changed()
          return added
        }
        case 'library_remove_folder': folders = folders.filter((f) => f.id !== a.id); await changed(); return null
        case 'library_set_folder_enabled': folders = folders.map((f) => (f.id === a.id ? { ...f, enabled: a.enabled as boolean } : f)); await changed(); return folders.find((f) => f.id === a.id)
        case 'library_rename_folder': folders = folders.map((f) => (f.id === a.id ? { ...f, name: a.name as string } : f)); await changed(); return folders.find((f) => f.id === a.id)
        case 'sync_not_on_share': return onlyHere
        case 'sync_copies': return copies
        case 'sync_clear_copies': { const n = copies.songs; copies = { songs: 0, bytes: 0 }; return n }
        case 'sync_move_to_sync': {
          const ids = (a.items as { id: number }[]).map((i) => i.id)
          onlyHere = onlyHere.filter((s) => !ids.includes(s.id))
          await wait(800)
          await changed()
          return { moved: ids.length, alreadyThere: 0, failed: [], cancelled: false }
        }
        case 'plugin:dialog|open': return 'E:\Downloads\New music'
        case 'plugin:native-audio|get_state':
        case 'plugin:native-audio|initialize': return playerState
        case 'plugin:native-audio|get_queue': return playerQueue()
        case 'plugin:native-audio|dispose':
          queueItems = []
          playerState = { ...playerState, isPlaying: false, queueIndex: -1, queueLength: 0, currentId: 0, currentTime: 0, duration: 0 }
          return null
        case 'plugin:native-audio|play': playerState = { ...playerState, isPlaying: true }; return playerState
        case 'plugin:native-audio|pause': playerState = { ...playerState, isPlaying: false }; return playerState
        case 'plugin:native-audio|next': return atIndex(Math.min(queueItems.length - 1, playerState.queueIndex + 1))
        case 'plugin:native-audio|previous': return atIndex(Math.max(0, playerState.queueIndex - 1))
        case 'plugin:native-audio|skip_to': return atIndex(a.index as number)
        case 'plugin:native-audio|seek_to': playerState = { ...playerState, currentTime: a.position as number }; return playerState
        case 'plugin:native-audio|set_volume': playerState = { ...playerState, volume: a.volume as number }; return playerState
        case 'plugin:native-audio|set_shuffle': playerState = { ...playerState, shuffle: a.enabled as boolean }; return playerState
        case 'plugin:native-audio|set_repeat_mode': playerState = { ...playerState, repeatMode: a.mode as string }; return playerState
        case 'plugin:native-audio|remove_from_queue': {
          queueItems = queueItems.filter((_, i) => i !== a.index)
          playerState = { ...playerState, queueLength: queueItems.length }
          return playerState
        }
        // The preview doesn't shuffle: the play order is the list.
        case 'plugin:native-audio|move_in_play_order':
        case 'plugin:native-audio|move_in_queue': {
          const [moved] = queueItems.splice(a.from as number, 1)
          queueItems.splice(a.to as number, 0, moved)
          return playerState
        }
        case 'library_liked_ids': return [202]
        case 'library_tracks_by_ids': return queueTracks(a.ids as number[])
        case 'library_track_artists': return Object.fromEntries(queueItems.filter((i) => (a.ids as number[]).includes(i.id)).map((i) => [i.id, [{ id: 3, name: i.artist }]]))
        case 'library_queue_follow': return queueFollow
        case 'library_set_queue_follow': queueFollow = a.settings as QueueFollow; return queueFollow
        case 'library_player_settings': return player
        case 'library_set_player_settings': player = a.settings as PlayerSettings; return player
        case 'plugin:native-audio|get_output_devices': return outputs
        case 'plugin:native-audio|set_output_device': {
          const id = a.id as string | null
          outputs = { ...outputs, selected: id, active: id ?? outputs.devices.find((d) => d.isDefault)!.id }
          return outputs
        }
        default: return null
      }
    },
    { shouldMockEvents: true },
  )
  console.info('melue: browser preview, with sample data (status:', status + ')')
}
