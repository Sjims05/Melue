import { isAndroid, isMobile } from './platform'

/**
 * What this platform can do, so shared code and the UI can hide what doesn't apply
 * instead of checking the platform themselves.
 */
export interface Capabilities {
  /** A player exists. The desktop player isn't built yet. */
  player: boolean
  /** Playback keeps going with the app in the background or the screen off (a native media service). */
  backgroundPlayback: boolean
  /** Android Auto (CarPlay later): the car library and the car's buttons. */
  carMode: boolean
  /** The music folder can be on an SD card or other storage the user picks through Android's picker. */
  removableStorage: boolean
}

export const capabilities: Capabilities = {
  player: isMobile,
  backgroundPlayback: isMobile,
  carMode: isAndroid,
  removableStorage: isAndroid,
}
