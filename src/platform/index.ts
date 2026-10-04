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

/** The player: the native-audio plugin on every platform (Media3 on Android, its Rust player on desktop). */
export const player: Player = new NativePlayer()

/** Picking and reaching the music folder on this platform. */
export const storage: MusicStorage = isAndroid ? new AndroidStorage() : new DesktopStorage()

/** Lists kept as things play (recently played, ...): by the native-audio plugin on every platform. */
export const trackedLists: TrackedLists = new NativeTrackedLists()
