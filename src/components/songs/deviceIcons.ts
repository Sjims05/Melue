/**
 * "This device" and "Not on this device yet" as icons, beside a song's title: the device Melue runs on (a phone or a
 * screen), plain or slashed. Lucide has a slashed screen (MonitorOff) but no slashed phone, so it's drawn here the way
 * Lucide draws its slashed icons (MonitorOff's): the slash from corner to corner; the outline on its lower side runs
 * into it, the one on its clear side stops 4 units (square to the slash) short of it.
 */
import { createLucideIcon, Monitor, MonitorOff, Smartphone } from '@lucide/vue'
import { isMobile } from '@/platform'

export const SmartphoneOff = createLucideIcon({
  name: 'smartphone-off',
  size: 24,
  node: [
    ['path', { d: 'M19 19v1a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5', key: 'lower' }],
    ['path', { d: 'M7.656 2H17a2 2 0 0 1 2 2v9.344', key: 'upper' }],
    ['path', { d: 'M12 18h.01', key: 'button' }],
    ['path', { d: 'm2 2 20 20', key: 'slash' }],
  ],
})

/**
 * Spotify's logo (a circle, three arcs) slashed, for a song in a Spotify playlist that isn't on Spotify ("Only in
 * Melue"). Lucide has no brand icons; drawn the way it slashes its own (CircleOff: the circle and arcs on the slash's
 * lower side run into it, those on its clear side stop 4 units short).
 */
export const SpotifyOff = createLucideIcon({
  name: 'spotify-off',
  size: 24,
  node: [
    ['path', { d: 'M8.35 2.69A10 10 0 0 1 21.3 15.65', key: 'upper-circle' }],
    ['path', { d: 'M19.08 19.08A10 10 0 1 1 4.92 4.92', key: 'lower-circle' }],
    ['path', { d: 'm2 2 20 20', key: 'slash' }],
    ['path', { d: 'M6.4 9.4C7.22 9.16 8.05 8.99 8.88 8.88', key: 'top-left' }],
    ['path', { d: 'M15 9.34C15.9 9.61 16.77 9.96 17.6 10.4', key: 'top-right' }],
    ['path', { d: 'M7.2 12.9C8.98 12.44 10.83 12.34 12.61 12.61', key: 'middle' }],
    ['path', { d: 'M8 16.2C10.4 15.7 12.9 15.9 15 16.9', key: 'bottom' }],
  ],
})

/**
 * Lucide's file slashed, for a song in a playlist that follows a file but isn't in it ("Only in Melue"), drawn as
 * SmartphoneOff is: the slash runs through the file's top-left and bottom-right corners; the outline on its lower side
 * runs into it, the one on its clear side (and the folded corner) stops 4 units short of it.
 */
export const FileOff = createLucideIcon({
  name: 'file-off',
  size: 24,
  node: [
    ['path', { d: 'M20 20a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4', key: 'lower' }],
    ['path', { d: 'M7.66 2H14a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v6.34', key: 'upper' }],
    ['path', { d: 'M14 2v5a1 1 0 0 0 1 1h5', key: 'fold' }],
    ['path', { d: 'm2 2 20 20', key: 'slash' }],
  ],
})

/** Only on this device (not on the share). */
export const ThisDeviceIcon = isMobile ? Smartphone : Monitor
/** Not on this device yet (a synced playlist's song not synced here). */
export const NotHereIcon = isMobile ? SmartphoneOff : MonitorOff

/** The note a song not on this device yet has: shown as NotHereIcon, the words its label. */
export const NOT_HERE = 'Not on this device yet'
