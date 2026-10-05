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
}

/** What started a sync. */
export type SyncTrigger = 'start' | 'return' | 'changed' | 'periodic' | 'button'

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
}

export const playbackSync = {
  settings: () => invoke<PlaybackSettings>('playback_sync_settings'),
  setSettings: (settings: PlaybackSettings) => invoke<PlaybackSettings>('playback_sync_set_settings', { settings }),
  /** Another device's playback waiting for an answer, if any. */
  offer: () => invoke<PlaybackFile | null>('playback_offer'),
  /** Calls `listener` when another device's playback is offered. Resolves to a function that stops listening. */
  onOffer: (listener: (offer: PlaybackFile) => void) => listen<PlaybackFile>('playback-offer', (e) => listener(e.payload)),
  /** Calls `listener` when another device's playback was loaded without asking (the "follow" setting). */
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
