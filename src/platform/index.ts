/**
 * The one place that knows which platform the app runs on. Everything else imports from here
 * and gets the right version of each service, without checking the platform itself.
 *
 * The platform is fixed at build time, so the unused side of each choice below (and what it imports)
 * is left out of the build: the desktop app doesn't contain the Android storage picker, for example.
 */
import { isAndroid } from './platform'
import type { Player } from './player/Player'
import { NativePlayer } from './player/NativePlayer'
import { NativeTrackedLists } from './trackedLists/NativeTrackedLists'
import type { TrackedLists } from './trackedLists/TrackedLists'
import { AndroidStorage } from './storage/AndroidStorage'
import { DesktopStorage } from './storage/DesktopStorage'
import type { MusicStorage } from './storage/MusicStorage'

export * from './platform'
export { capabilities, type Capabilities } from './capabilities'
export type * from './player/Player'
export type { MusicStorage } from './storage/MusicStorage'
export type * from './trackedLists/TrackedLists'

/**
 * The player: the native-audio plugin on every platform (Media3 on Android, AVPlayer on iOS, its
 * Rust player on desktop). Typed as nullable so a platform without one can be added later.
 */
export const player: Player | null = new NativePlayer()

/** Picking and reaching the music folder on this platform. */
export const storage: MusicStorage = isAndroid ? new AndroidStorage() : new DesktopStorage()

/**
 * Lists kept as things play (recently played, ...): by the native-audio plugin on every platform.
 * Typed as nullable so a platform without them can be added later.
 */
export const trackedLists: TrackedLists | null = new NativeTrackedLists()
