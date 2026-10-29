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

/** Only on this device (not on the share). */
export const ThisDeviceIcon = isMobile ? Smartphone : Monitor
/** Not on this device yet (a synced playlist's song not synced here). */
export const NotHereIcon = isMobile ? SmartphoneOff : MonitorOff

/** The note a song not on this device yet has: shown as NotHereIcon, the words its label. */
export const NOT_HERE = 'Not on this device yet'
