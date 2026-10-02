/**
 * The one place that knows which platform the app runs on. Everything else imports from here
 * and gets the right version of each service, without checking the platform itself.
 *
 * The platform is fixed at build time, so the unused side of each choice below (and what it imports)
 * is left out of the build: the desktop app doesn't contain the native-audio plugin.
 */
import { isAndroid, isMobile } from './platform'
import type { Player } from './player/Player'
import { NativePlayer } from './player/NativePlayer'
import { AndroidStorage } from './storage/AndroidStorage'
import { DesktopStorage } from './storage/DesktopStorage'
import type { MusicStorage } from './storage/MusicStorage'

export * from './platform'
export { capabilities, type Capabilities } from './capabilities'
export type * from './player/Player'
export type { MusicStorage } from './storage/MusicStorage'

/** The player for this platform, or null where there isn't one yet (desktop: the desktop player comes later). */
export const player: Player | null = isMobile ? new NativePlayer() : null

/** Picking and reaching the music folder on this platform. */
export const storage: MusicStorage = isAndroid ? new AndroidStorage() : new DesktopStorage()
