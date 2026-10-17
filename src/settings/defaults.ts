/**
 * The settings' defaults, for "Reset to default" (a row's right-click or long-press menu). They match Melue's own
 * (src-tauri: sync/mod.rs SyncSettings::default, handoff.rs PlaybackSettings::default); keep them in step.
 */
import type { PlaybackSettings, SyncSettings } from '@/library/sync'

export const SYNC_DEFAULTS: SyncSettings = {
  removeGone: false,
  askBeforeRemoving: 5,
  atStart: true,
  onReturnAfterMinutes: 15,
  onChange: true,
  everyMinutes: null,
  copyOnMetered: false,
  moveNewSongs: false,
  shareLayout: 'artistAlbum',
}

/** An empty name goes back to the system's name. */
export const PLAYBACK_DEFAULTS: PlaybackSettings = { mode: 'ask', deviceName: '', continueWithinMinutes: 10 }
