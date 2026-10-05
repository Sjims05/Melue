/**
 * Whether sync is available: a share is set up (Phase 2's share setup). Screens show sync buttons and
 * options only when `sync.available` is true (the sync code itself is always there). It's live: it
 * follows a share being set up or removed while the app runs.
 *
 * Whether the platform can do something (Android Auto, background playback) is in `capabilities`
 * instead: that never changes while the app runs; this does.
 */
import { invoke } from '@tauri-apps/api/core'
import { reactive } from 'vue'
import { library, type FolderKind, type MusicFolder } from '.'

export const sync = reactive({
  /** A share is set up: show the sync buttons and options. */
  available: false,
})

let started = false

/** Once at app start: reads it, then follows its changes. */
export async function startSync(): Promise<void> {
  if (started) return
  started = true
  await library.onChange((change) => {
    if (change.reason === 'sync') refresh().catch(() => {})
  })
  await refresh()
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
