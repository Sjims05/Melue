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
    : { kind: 'smb', host: 'nas', share: 'music', folder: '', user: 'me', passwordStore: 'secure', syncFolderId: 3 },
  syncFolder: null,
  needsPassword: status === 'password',
  defaultSyncFolder: 'C:\\Users\\me\\Music\\Melue',
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
    albumId: 1, path: `${i.album}/${i.title}.mp3`, addedAt: 0, artists: [{ id: 3, name: i.artist }],
  }))

// The Library's songs: the queue's, and many more made up (to scroll a big library), sorted as the Songs tab asks.
const WORDS = ['Amber', 'Blue', 'Cold', 'Distant', 'Echo', 'Fading', 'Golden', 'Hollow', 'Island', 'Jade', 'Kindred', 'Lunar', 'Midnight',
  'Northern', 'Open', 'Paper', 'Quiet', 'River', 'Silver', 'The Tide', 'Upper', 'Velvet', 'Winter', 'Yellow', 'Zero', '99 Lights']
const BIG = Array.from({ length: 1500 }, (_, i) => {
  const title = `${WORDS[(i * 7) % WORDS.length]} ${['days', 'roads', 'hearts', 'signals', 'rooms', 'waves'][i % 6]} ${i}`
  const artist = `${WORDS[(i * 3) % WORDS.length]} ${['Band', 'Collective', 'Trio'][i % 3]}`
  return { id: 1000 + i, title, artist, album: i % 5 ? `${WORDS[(i * 11) % WORDS.length]} sessions` : null, plays: (i * 37) % 90, added: i }
})
function bigLibrary(sort: string) {
  const songs = [...queueTracks(queueItems.map((q) => q.id)), ...BIG.map((s) => ({
    ...queueTracks([201])[0], id: s.id, title: s.title, artist: s.artist, album: s.album, albumId: s.album ? 1 : null, playCount: s.plays,
    addedAt: s.added, liked: s.id % 13 === 0, artists: [{ id: 3, name: s.artist }],
  }))]
  const key = (t: (typeof songs)[number]) =>
    sort === 'artist' ? t.artist ?? '' : sort === 'album' ? t.album ?? '~' : t.title.replace(/^the\s+/i, '')
  if (sort === 'added') return songs.sort((x, y) => y.addedAt - x.addedAt)
  if (sort === 'plays') return songs.sort((x, y) => y.playCount - x.playCount)
  return songs.sort((x, y) => key(x).localeCompare(key(y)))
}

// The Library's albums: the made-up songs' albums, numbered.
function bigAlbums() {
  const titles = [...new Set(bigLibrary('album').map((t) => t.album).filter((t): t is string => !!t))]
  return titles.map((title, i) => ({
    id: i + 1, title, artist: i % 7 ? `${title.split(' ')[0]} Band` : null, artistId: i % 7 ? 3 : null, compilation: !(i % 7),
    year: 2000 + (i % 24), cover: null, tracks: bigLibrary('album').filter((t) => t.album === title).length,
  }))
}

// Your playlists: synced, only here, linked, one with a missing song, one waiting for a choice.
const playlist = (id: number, name: string, extra: Record<string, unknown>) => ({
  id, name, kind: 'app', entries: 8, available: 8, suggested: 0, removedHere: 0, file: null, fileMissing: false, newInFile: 'added',
  removedFromFile: 'removed', writeBack: false, following: true, durationMs: 31 * 60_000, synced: true, localOnly: false, notHere: 0, covers: [],
  createdAt: 0, updatedAt: 0, ...extra,
})
let playlists = [
  playlist(10, 'Night Drive', {}),
  playlist(11, 'Focus', { synced: false, localOnly: true }),
  playlist(12, 'Road trip', { entries: 9, available: 8 }),
  playlist(13, 'Melue test', { kind: 'linked', synced: false, file: 'Melue test.m3u' }),
]

// Playlists: the auto ones (one hidden), and two deleted lately.
let autoPlaylists: AutoPlaylist[] = [
  { source: 'songs', name: 'All songs', kind: 'songs', songs: 1508, size: null, hidden: false },
  { source: 'liked', name: 'Liked songs', kind: 'liked', songs: 86, size: null, hidden: false },
  { source: 'top:month', name: 'Top songs: last month', kind: 'top', songs: 50, size: 50, hidden: true },
  { source: 'top:year', name: 'Top songs: last year', kind: 'top', songs: 50, size: 50, hidden: false },
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
  folder(1, 'Music', 'C:\\Users\\me\\Music', 1012),
  folder(2, 'Music_Test', 'D:\Music_Test', 4),
  folder(3, 'Synced music', 'C:\\Users\\me\\Music\\Melue', 218, true),
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
        case 'library_playlists': return playlists
        case 'library_playlist': return playlists.find((p) => p.id === a.id)
        case 'library_playlists_with_track': return [11]
        case 'library_add_to_playlist': return [Date.now()]
        case 'library_set_playlist_following': playlists = playlists.map((p) => (p.id === a.id ? { ...p, following: a.following as boolean } : p)); return null
        case 'library_deleted_linked_playlists': return [{ id: 1, name: 'Old mix', songs: 14, deletedAt: Date.now() - 3 * 86_400_000, file: 'Old mix.m3u' }]
        case 'library_let_go_files': return [{ folderId: 1, folder: 'Music', path: 'Lists/Old mix.m3u' }]
        case 'library_playlist_entries': {
          const songs = bigLibrary('title').slice(0, 8)
          const states = ['both', 'both', 'suggested', 'both', 'removed_here', 'both', 'file_removed', 'both']
          const linked = playlists.find((p) => p.id === a.id)?.kind === 'linked'
          return [
            ...songs.map((t, i) => ({ id: 500 + i, position: i, addedAt: 0, state: linked ? states[i] : 'both', fileEntry: null, syncTitle: null, syncArtist: null, track: { ...t, available: i !== 3 } })),
            { id: 600, position: 8, addedAt: 0, state: 'both', fileEntry: null, syncTitle: 'Song from the phone', syncArtist: 'Phone Band', track: null },
          ]
        }
        case 'library_move_in_playlist': case 'library_remove_from_playlist': case 'library_restore_playlist_entry': return null
        case 'library_create_playlist': {
          const made = { ...playlists[0], id: Date.now(), name: a.name as string, kind: 'app', entries: 0, available: 0, durationMs: 0, covers: [], synced: false, localOnly: true, notHere: 0 }
          playlists = [made, ...playlists]
          return made
        }
        case 'library_rename_playlist': playlists = playlists.map((p) => (p.id === a.id ? { ...p, name: a.name as string } : p)); return playlists.find((p) => p.id === a.id)
        case 'library_delete_playlist': playlists = playlists.filter((p) => p.id !== a.id); return null
        case 'playlist_conflicts': return [{ syncId: 's1', kind: 'changed', name: 'Road trip', deviceName: 'Phone', playlistId: 12, foundAt: 0 }]
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
        case 'library_tracks': return bigLibrary(a.sort as string).slice(a.offset as number, (a.offset as number) + (a.limit as number))
        case 'library_albums': return bigAlbums()
        case 'library_auto_playlist_tracks': return a.source === 'songs' ? bigLibrary('title') : bigLibrary('plays').slice(0, 25)
        case 'library_folder_tracks': return bigLibrary('album').map((t) => ({ ...t, path: `${t.album ?? ''}${t.album ? '/' : ''}${t.title}.mp3` }))
        case 'library_artists': {
          const names = [...new Set(bigLibrary('artist').map((t) => t.artist as string))]
          return names.map((name, i) => ({ id: i + 1, name, tracks: bigLibrary('artist').filter((t) => t.artist === name).length, featured: i % 4, albums: i % 3 }))
        }
        case 'library_artist_page': {
          const name = [...new Set(bigLibrary('artist').map((t) => t.artist as string))][(a.id as number) - 1]
          if (!name) return null
          const tracks = bigLibrary('album').filter((t) => t.artist === name)
          const albums = bigAlbums().filter((al) => tracks.some((t) => t.album === al.title)).slice(0, 4)
          return { artist: { id: a.id, name, tracks: tracks.length, featured: 2, albums: albums.length }, albums, tracks, featured: bigLibrary('title').slice(0, 2) }
        }
        case 'library_album_page': {
          const album = bigAlbums().find((al) => al.id === a.id)
          if (!album) return null
          const tracks = bigLibrary('title').filter((t) => t.album === album.title)
          return { album, tracks: tracks.map((t, i) => ({ ...t, albumId: album.id, trackNo: (i % 12) + 1, discNo: i < 12 ? 1 : 2 })) }
        }
        case 'library_queue_items_for': return queueItems.filter((i) => (a.ids as number[]).includes(i.id))
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
