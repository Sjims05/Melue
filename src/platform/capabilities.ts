import { isAndroid, isMobile } from './platform'

/**
 * What this platform can do, so shared code and the UI can hide what doesn't apply
 * instead of checking the platform themselves. These never change while the app runs; whether sync is
 * available (a share is set up) can, so it's in '@/library/sync' instead.
 */
export interface Capabilities {
  /**
   * Playback keeps going after the app is closed or the screen is off (a native media service), as on
   * Android. On desktop it plays while the app runs, also minimized, and stops when it's closed.
   */
  backgroundPlayback: boolean
  /** Android Auto (CarPlay later): the car library and the car's buttons. */
  carMode: boolean
  /** The music folder can be on an SD card or other storage the user picks through Android's picker. */
  removableStorage: boolean
}

export const capabilities: Capabilities = {
  backgroundPlayback: isMobile,
  carMode: isAndroid,
  removableStorage: isAndroid,
}
