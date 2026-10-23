/**
 * Connect device (library/remote.ts; src-tauri remote/link.rs): where this device's music plays, this device or
 * another of yours, and moving it. Playing on another: what plays here goes there (from where it is) and this one
 * pauses; "This device" brings it back the same way; Disconnect leaves it playing there. The other device's state
 * comes in as it changes. While connected, Continue (synced playback) rests on both devices.
 */
import { Monitor, MonitorSpeaker, Smartphone, Unplug } from '@lucide/vue'
import { remote, type Device } from '@/library/remote'
import { openMenu } from '@/lib/menuDrawer'
import { toast } from '@/lib/toast'
import { syncNowPlaying } from './nowPlaying'
import { playingOn } from './playingOn'

export { playingOn }

/** The devices to offer: this one, then your others that are online or were seen in the last day. */
export async function devicesToOffer(): Promise<Device[]> {
  const all = await remote.devices().catch(() => [] as Device[])
  return all.filter((d) => d.this || d.online || Date.now() - d.lastSeenMs < 86_400_000)
}

/** A device's line in the menu: online or not, and when it asks first or doesn't let others play. */
export function deviceNote(d: Device): string {
  if (d.this) return playingOn.device ? '' : 'Playing here'
  if (d.allow === 'off') return 'Doesn’t let others play on it'
  return [d.online ? '' : 'Not seen just now', d.allow === 'ask' ? 'asks first' : ''].filter(Boolean).join(' · ')
}

/** Plays on `device` (this one: brings it back here). */
export async function playOn(device: Device) {
  if (device.this) return bringBack()
  if (playingOn.device?.id === device.id || playingOn.connecting) return
  playingOn.connecting = device.id
  if (device.allow === 'ask') toast(`Asking ${device.name}…`, MonitorSpeaker)
  try {
    const connected = await remote.playOn(device.id)
    playingOn.device = { id: connected.id, name: connected.name }
    await syncNowPlaying().catch(() => {})
    toast(`Playing on ${connected.name}`, MonitorSpeaker)
  } catch (e) {
    toast(`Couldn’t play on ${device.name}: ${e}`, MonitorSpeaker)
  } finally {
    playingOn.connecting = null
  }
}

/** Back to this device: what plays there plays here, and it pauses there. */
export async function bringBack() {
  if (!playingOn.device) return
  const name = playingOn.device.name
  try {
    await remote.bringBack()
    toast(`Playing on this device (from ${name})`, MonitorSpeaker)
  } catch (e) {
    toast(`Couldn’t bring it back from ${name}: ${e}`, MonitorSpeaker)
  } finally {
    playingOn.device = null
    playingOn.state = null
    await syncNowPlaying().catch(() => {})
  }
}

/** The other way round: the devices playing on this one are disconnected, and the music goes on here. */
export async function disconnectPlayedFrom() {
  const names = playingOn.playedFrom.join(' and ')
  await remote.endPlayedFrom().catch(() => {})
  playingOn.playedFrom = []
  if (names) toast(`Disconnected ${names}: the music goes on here`, Unplug)
}

/** Disconnect: the other device goes on playing; this one stops controlling it (and Continue is back). */
export async function disconnect() {
  const name = playingOn.device?.name
  await remote.disconnect().catch(() => {})
  playingOn.device = null
  playingOn.state = null
  if (name) toast(`Disconnected: ${name} goes on playing`, Unplug)
}

/** Connect device on a phone: the devices in the menu drawer, the one playing ticked. */
export async function openDevicesDrawer() {
  const devices = (await devicesToOffer()).filter((d) => d.this || d.allow !== 'off')
  const playing = devices.find((d) => (d.this ? !playingOn.device : playingOn.device?.id === d.id))
  openMenu({
    title: 'Play on',
    subtitle: devices.length < 2 ? 'Your other devices show here when they run Melue with the same share' : undefined,
    items: [
      ...devices.map((d) => ({
        action: d.id,
        label: [d.this ? `This device (${d.name})` : d.name, deviceNote(d)].filter(Boolean).join(' · '),
        icon: d.kind === 'phone' ? Smartphone : Monitor,
      })),
      ...(playingOn.device ? [{ action: 'disconnect', label: `Disconnect (keep playing on ${playingOn.device.name})`, icon: Unplug, apart: true }] : []),
      ...(playingOn.playedFrom.length
        ? [{ action: 'disconnect-from', label: `Disconnect ${playingOn.playedFrom.join(' and ')} (keep playing here)`, icon: Unplug, apart: true }]
        : []),
    ],
    picked: playing?.id,
    pick: (id) => {
      if (id === 'disconnect') return disconnect()
      if (id === 'disconnect-from') return disconnectPlayedFrom()
      const device = devices.find((d) => d.id === id)
      if (device) playOn(device)
    },
  })
}

let started = false
/** Once (the app's frame): follows the other device, and picks up a connection made before a reload. */
export async function startRemotePlayback() {
  if (started) return
  started = true
  await remote.onState((state) => (playingOn.state = state)).catch(() => {})
  await remote.onError((error) => toast(error, MonitorSpeaker)).catch(() => {})
  await remote
    .onClosed((why) => {
      const name = playingOn.device?.name
      playingOn.device = null
      playingOn.state = null
      if (why && name) toast(`Stopped playing on ${name}: ${why}`, MonitorSpeaker)
    })
    .catch(() => {})
  await remote.onPlayedFrom((names) => (playingOn.playedFrom = names)).catch(() => {})
  const settings = await remote.settings().catch(() => null)
  if (settings) playingOn.phoneStep = settings.phoneStep
  playingOn.playedFrom = await remote.playedFrom().catch(() => [])
  const now = await remote.status().catch(() => null)
  if (now) {
    playingOn.device = { id: now[0].id, name: now[0].name || 'another device' }
    playingOn.state = now[1]
  }
}
