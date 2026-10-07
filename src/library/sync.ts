/**
 * Whether sync is available: a share is set up (Phase 2's share setup). Screens show sync buttons and
 * options only when `sync.available` is true (the sync code itself is always there). It's live: it
 * follows a share being set up or removed while the app runs.
 *
 * Whether the platform can do something (Android Auto, background playback) is in `capabilities`
 * instead: that never changes while the app runs; this does.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { reactive } from 'vue'
import { library, type FolderKind, type MusicFolder } from '.'

export const sync = reactive({
  /** A share is set up: show the sync buttons and options. */
  available: false,
  /** Whether a sync is running (and how far), the last sync, the last problem. Live. */
  status: null as SyncStatus | null,
})

let started = false

/** Once at app start: reads it, then follows its changes. */
export async function startSync(): Promise<void> {
  if (started) return
  started = true
  await library.onChange((change) => {
    if (change.reason === 'sync') refresh().catch(() => {})
  })
  await listen<SyncStatus>('sync-status', (e) => {
    sync.status = e.payload
  })
  await refresh()
  sync.status = await invoke<SyncStatus>('sync_status').catch(() => null)
}

async function refresh() {
  sync.available = await invoke<boolean>('library_sync_available')
}

// ---- setting up the share (src-tauri/src/share/setup.rs)

/** Where the share's password is kept: the system's secure store, only in memory (asked after each start), or a plain file. */
export type PasswordStore = 'secure' | 'ask' | 'file'

/** Where the share is: an SMB share Melue logs in to, or (desktop) a network drive the system has connected. */
export type ShareSetup =
  | { kind: 'smb'; host: string; share: string; folder: string; user: string; passwordStore: PasswordStore }
  /** `stream`: played straight from the drive (it becomes the synced library folder), not copied. */
  | { kind: 'drive'; path: string; stream: boolean }

/** The share as set up (never the password). */
export type ShareSettings = ShareSetup & { syncFolderId: number }

/** Where the synced music goes: desktop a path; Android a folder chosen with the picker. */
export interface SyncFolderChoice {
  kind: FolderKind
  location: string
}

export type ShareError =
  | { kind: 'unreachable' | 'login' | 'full' | 'other'; detail: string }
  | { kind: 'notFound' | 'noPermission'; path: string }

/** Why setting up or using the share didn't work. 'noSecureStore': ask each time, or keep it in a plain file? */
export type SetupError =
  | { kind: 'share'; error: ShareError }
  | { kind: 'noSecureStore'; detail: string }
  | { kind: 'needPassword' }
  | { kind: 'notSetUp' }
  | { kind: 'other'; detail: string }

export interface ShareInfo {
  settings: ShareSettings | null
  syncFolder: MusicFolder | null
  /** The password has to be given before the share can be used (asked each time, after a start). */
  needsPassword: boolean
  /** Desktop: where the synced music goes unless another folder is chosen (Music/Melue). */
  defaultSyncFolder: string | null
}

/** A setup error in words. */
export function describeSetupError(e: unknown): string {
  const error = e as SetupError
  switch (error?.kind) {
    case 'share': {
      const s = error.error
      if (s.kind === 'unreachable') return `Can't reach the share (is it on, and the VPN connected?): ${s.detail}`
      if (s.kind === 'login') return `The share didn't accept the user name or password`
      if (s.kind === 'notFound') return `Not found on the share: ${s.path}`
      if (s.kind === 'noPermission') return `No permission on the share for: ${s.path}`
      return `${s.kind === 'full' ? 'Out of space' : 'Share error'}: ${s.kind === 'full' || s.kind === 'other' ? s.detail : ''}`
    }
    case 'noSecureStore':
      return `This system has no secure place for the password (${error.detail}): ask for it each time, or keep it in a plain file?`
    case 'needPassword':
      return "The share's password is needed"
    case 'notSetUp':
      return 'No share is set up'
    case 'other':
      return error.detail
    default:
      return String(e)
  }
}

export const share = {
  info: () => invoke<ShareInfo>('share_info'),
  /** Tries a share without setting it up ("Test connection"). Rejects with a SetupError. */
  test: (setup: ShareSetup, password?: string) => invoke<void>('share_test', { setup, password }),
  /**
   * Sets up the share (tried first) and the synced folder: desktop Music/Melue when `syncFolder` is left
   * out; not needed for a drive played straight from. Rejects with a SetupError.
   */
  setUp: (setup: ShareSetup, password?: string, syncFolder?: SyncFolderChoice) =>
    invoke<ShareInfo>('share_set_up', { setup, password, syncFolder }),
  /** A new password (changed, or asked for after a start), tried first; kept in `store` or where it was. */
  setPassword: (password: string, store?: PasswordStore) => invoke<ShareInfo>('share_set_password', { password, store }),
  /** Removes the share: the synced folder stays as a normal folder, with its songs. */
  remove: () => invoke<ShareInfo>('share_remove'),
  /** Android: shows the synced music in other apps (default) or hides it (a .nomedia file). */
  setShownInOtherApps: (shown: boolean) => invoke<void>('share_set_shown_in_other_apps', { shown }),
}

// ---- the sync (src-tauri/src/sync)

export type SyncFileKind = 'song' | 'playlist' | 'cover'

/** Why a file is copied: new on the share, deleted here, changed on the share, or changed here (undone). */
export type SyncWhy = 'new' | 'deletedHere' | 'changedThere' | 'changedHere'

export interface PlanFile {
  path: string
  kind: SyncFileKind
  size: number
  why: SyncWhy
}

/** What a sync would do (paths relative to the music folder). Nothing is changed by planning. */
export interface SyncPlan {
  download: PlanFile[]
  update: PlanFile[]
  /** Here already, the same as on the share: only remembered. */
  adopt: string[]
  /** Moved or renamed on the share: the copy here follows (no download). */
  moves: { from: string; to: string }[]
  /** Synced before, gone from the share: kept here, unless `removeGone`. */
  gone: string[]
  /** Put here by hand: stays. */
  onlyHere: string[]
  forget: string[]
  removeGone: boolean
  /** Removing that many needs the user's OK first. */
  needsConfirmation: boolean
  unchanged: number
  /** How much would be copied. */
  bytes: number
}

export interface SyncSettings {
  /** Remove the copies of songs gone from the share (default off: kept, listed for a clean-up). */
  removeGone: boolean
  /** Ask before removing more than this many songs (0: always ask). Default 5. */
  askBeforeRemoving: number
  /** Sync when Melue starts (default on). */
  atStart: boolean
  /** Sync when the user comes back to the app if the last sync is this many minutes ago (default 15; null: no). */
  onReturnAfterMinutes: number | null
  /** Sync when the share says its music changed, while Melue runs (default on). */
  onChange: boolean
  /** Also sync every this many minutes while Melue runs (default null: no). */
  everyMinutes: number | null
  /** Copy songs on metered connections (mobile data, a hotspot); off: they wait for Wi-Fi (default off). */
  copyOnMetered: boolean
  /** Move songs new in the other music folders to sync by themselves (default off). */
  moveNewSongs: boolean
  /** How songs are laid out on the share: Artist/Album/ (default) or all in one folder. */
  shareLayout: ShareLayout
}

export type ShareLayout = 'artistAlbum' | 'oneFolder'

/** What started a sync. */
export type SyncTrigger = 'start' | 'return' | 'changed' | 'periodic' | 'button' | 'retry' | 'network'

export interface LastSync {
  atMs: number
  trigger: SyncTrigger
  result: SyncResult
}

export interface SyncStatus {
  /** A sync is running: how far it is. */
  syncing: SyncProgress | null
  last: LastSync | null
  /** Why the last try didn't sync (cleared by the next sync). */
  problem: SyncFailure | null
  /** The share couldn't be reached: when it's tried again (sooner if the network changes). */
  retryAtMs: number | null
}

/** How far a running sync is. */
export interface SyncProgress {
  filesDone: number
  filesTotal: number
  bytesDone: number
  bytesTotal: number
  /** The file being copied (null at the end). */
  current: string | null
}

/** What a sync did. */
export interface SyncResult {
  copied: number
  bytes: number
  moved: number
  adopted: number
  removed: number
  /** Files that couldn't be copied (tried again next sync): [path, why]. */
  failed: [string, string][]
  /** Files not copied because the connection is metered (they wait for Wi-Fi). */
  waitingForWifi: number
  /** Songs gone from the share waiting for the user's OK to be removed (syncing.now(true) removes them). */
  removalsWaiting: number
  cancelled: boolean
  /** Why it stopped before the end (the share lost, the device full). */
  stopped: string | null
}

/** Why Sync now didn't sync. */
export type SyncFailure =
  | { kind: 'setup'; error: SetupError }
  | { kind: 'notEnoughSpace'; needed: number; free: number }
  | { kind: 'running' }
  | { kind: 'other'; detail: string }

export function describeSyncFailure(e: unknown): string {
  const failure = e as SyncFailure
  const gb = (bytes: number) => `${(bytes / 1e9).toFixed(1)} GB`
  switch (failure?.kind) {
    case 'setup':
      return describeSetupError(failure.error)
    case 'notEnoughSpace':
      return `Not enough free space: needs ${gb(failure.needed)}, ${gb(failure.free)} free`
    case 'running':
      return 'A sync is running already'
    case 'other':
      return failure.detail
    default:
      return String(e)
  }
}

export const syncing = {
  /** What a sync would do now (the dry run). Rejects with a SetupError. */
  plan: () => invoke<SyncPlan>('sync_plan'),
  /**
   * Syncs now (the library then scans the synced folder). `confirmedRemoval`: yes to removing more songs gone
   * from the share than the setting allows. Rejects with a SyncFailure.
   */
  now: (confirmedRemoval = false) => invoke<SyncResult>('sync_now', { confirmedRemoval }),
  /** Stops a running sync after the file it's copying. */
  cancel: () => invoke<void>('sync_cancel'),
  status: () => invoke<SyncStatus>('sync_status'),
  settings: () => invoke<SyncSettings>('sync_settings'),
  setSettings: (settings: SyncSettings) => invoke<SyncSettings>('sync_set_settings', { settings }),
}

// ---- synced playback: carrying on where another device left off

/** Another device's playback: its queue (songs by their path on the share), the song, where in it. */
export interface PlaybackFile {
  deviceId: string
  deviceName: string
  savedAtMs: number
  queue: string[]
  index: number
  positionS: number
  playing: boolean
  shuffle: boolean
  repeat: 'off' | 'all' | 'one'
  title: string | null
  artist: string | null
}

/** Ask first ("Continue where you left off on ...?"), load it without asking, or off. */
export type PlaybackMode = 'ask' | 'follow' | 'off'

export interface PlaybackSettings {
  mode: PlaybackMode
  /** This device's name for the others (the computer's name or the phone's unless changed). */
  deviceName: string | null
  /**
   * Ask: continue without asking when the other device's playback is at most this many minutes old (most
   * likely a switch from it to this one). null: always ask. Default 10.
   */
  continueWithinMinutes: number | null
}

export const playbackSync = {
  settings: () => invoke<PlaybackSettings>('playback_sync_settings'),
  setSettings: (settings: PlaybackSettings) => invoke<PlaybackSettings>('playback_sync_set_settings', { settings }),
  /** Another device's playback waiting for an answer, if any. */
  offer: () => invoke<PlaybackFile | null>('playback_offer'),
  /** Calls `listener` when another device's playback is offered. Resolves to a function that stops listening. */
  onOffer: (listener: (offer: PlaybackFile) => void) => listen<PlaybackFile>('playback-offer', (e) => listener(e.payload)),
  /** Calls `listener` when another device's playback was loaded without asking ("follow", or recent enough). */
  onFollowed: (listener: (other: PlaybackFile) => void) => listen<PlaybackFile>('playback-followed', (e) => listener(e.payload)),
  /** Continue: the other device's queue, loaded paused at its position. */
  continue: () => invoke<void>('playback_continue'),
  /** Not now: not offered again (until that device plays something else). */
  notNow: () => invoke<void>('playback_not_now'),
  /** Looks at the other devices' playback now. */
  check: () => invoke<void>('playback_check'),
}

/** "1:23" for 83 seconds. */
export function minutesAndSeconds(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

// ---- library data across devices: likes, plays and playlists

/** A playlist changed on two devices in ways that don't merge, waiting for a choice. */
export interface PlaylistConflict {
  syncId: string
  /**
   * - changed: both changed it (reordered, or renamed differently)
   * - deletedThere: deleted on the other device, changed here
   * - deletedHere: deleted here, changed on the other device
   */
  kind: 'changed' | 'deletedThere' | 'deletedHere'
  name: string
  /** The device whose change it conflicts with. */
  deviceName: string
  /** The playlist here (null: deleted here). */
  playlistId: number | null
  foundAt: number
}

/**
 * The choice for a conflict:
 * - mine: this device's version (deleted here: deleted everywhere; deleted there: kept with this device's changes)
 * - theirs: the other device's (deleted there: deleted here too; deleted here: brought back with their changes)
 * - both (changed): this device's is kept as a copy, "<name> (<this device>)"
 */
export type ConflictChoice = 'mine' | 'theirs' | 'both'

/** A synced playlist deleted in the last 30 days. */
export interface DeletedPlaylist {
  syncId: string
  name: string
  songs: number
  deletedAt: number
}

export const librarySync = {
  /** Syncs likes, plays and playlists with the share now (it also runs by itself after changes). */
  now: () => invoke<void>('library_sync_now'),
  conflicts: () => invoke<PlaylistConflict[]>('playlist_conflicts'),
  /** Calls `listener` with the conflicts waiting whenever they change. Resolves to a function that stops listening. */
  onConflicts: (listener: (conflicts: PlaylistConflict[]) => void) =>
    listen<PlaylistConflict[]>('playlist-conflicts', (e) => listener(e.payload)),
  answer: (syncId: string, choice: ConflictChoice) => invoke<void>('playlist_answer_conflict', { syncId, choice }),
  deletedPlaylists: () => invoke<DeletedPlaylist[]>('deleted_playlists'),
  /** Restores a deleted playlist, everywhere. Resolves with its id. */
  restorePlaylist: (syncId: string) => invoke<number>('restore_playlist', { syncId }),
  /** "Only on this device": not synced (the other devices keep theirs), or synced again. */
  setPlaylistLocalOnly: (id: number, localOnly: boolean) => invoke<void>('set_playlist_local_only', { id, localOnly }),
}

// ---- moving songs to sync

/** A local song that isn't on the share. */
export interface LocalSong {
  id: number
  title: string
  artist: string | null
  album: string | null
  /** Its music folder's name. */
  folder: string
  size: number
}

/** What to move: a song in the library (its original goes after), or a file (copied: it stays where it is). */
export type ToMove = { kind: 'song'; id: number } | { kind: 'file'; path: string }

export interface MoveResult {
  /** Put on the share (and in the synced folder). */
  moved: number
  /** On the share already (the same song): only the original goes. */
  alreadyThere: number
  /** Couldn't be moved (left as they were): [title or file, why]. */
  failed: [string, string][]
  cancelled: boolean
}

/** Local copies of synced songs, hidden, waiting for a choice: clear the space, or keep them. */
export interface Copies {
  songs: number
  bytes: number
}

export const moving = {
  /** Local songs that aren't on the share (the "Not on your share" list). */
  notOnShare: () => invoke<LocalSong[]>('sync_not_on_share'),
  /**
   * Moves songs to sync: to the share (Artist/Album/), and into the synced folder here; a song's original goes
   * once its synced copy is in the library (Recycle Bin; on Android its trash, after Android asks). Its likes,
   * plays and playlist entries go with it. Progress shows in sync.status. Rejects with a SyncFailure.
   */
  moveToSync: (items: ToMove[]) => invoke<MoveResult>('sync_move_to_sync', { items }),
  copies: () => invoke<Copies>('sync_copies'),
  /** Calls `listener` when hidden copies wait for a choice. Resolves to a function that stops listening. */
  onCopies: (listener: (copies: Copies) => void) => listen<Copies>('song-copies', (e) => listener(e.payload)),
  /** Clear space: removes the hidden copies. Resolves with how many were removed (or handed to Android). */
  clearCopies: () => invoke<number>('sync_clear_copies'),
  /** Spike: looks for copies now, and says what it found (near misses: why a copy wasn't recognised). */
  check: () =>
    invoke<{ syncedSongs: number; otherSongs: number; hiddenNow: number; hidden: number; waiting: Copies; nearMisses: string[] }>(
      'sync_check_copies',
    ),
  /** Keeps the hidden copies (not asked about again). */
  keepCopies: () => invoke<void>('sync_keep_copies'),
}

// ---- the share's layout

export interface LayoutMove {
  from: string
  to: string
}

export interface LayoutStatus {
  layout: ShareLayout
  /** The share is in this layout: new songs are put in place by themselves. */
  applied: boolean
  /** Songs that would move. */
  toMove: number
  examples: LayoutMove[]
}

export interface ReorganiseResult {
  moved: number
  /** Playlist files (.m3u) and Melue playlists updated to the new paths. */
  playlistsUpdated: number
  failed: [string, string][]
}

export const shareLayout = {
  status: () => invoke<LayoutStatus>('sync_layout_status'),
  /** Puts the share in the chosen layout (the copies here move too; nothing is downloaded). Rejects with a SyncFailure. */
  reorganise: () => invoke<ReorganiseResult>('sync_reorganise'),
}
