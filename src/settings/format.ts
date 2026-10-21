/** How the settings put things into words: the share's name, times, sizes, what a sync did. */
import { ref } from 'vue'
import type { ShareSettings, SyncResult, SyncTrigger } from '@/library/sync'

/**
 * Where removed originals go, as the end of a sentence ("Then the original …"): the Recycle Bin; for good from a network
 * folder (`network`: any of them is); Android's trash. `many`: "them" instead of "it".
 */
export function whereOriginalsGo(isMobile: boolean, network: boolean, many: boolean): string {
  if (isMobile) return `is removed: Android asks you to move ${many ? 'them' : 'it'} to its trash (kept 30 days).`
  if (network) return 'is deleted for good: a network folder has no Recycle Bin.'
  return 'goes to the Recycle Bin.'
}

/** "me@nas/music", or a network drive's path. */
export function shareName(settings: ShareSettings | null | undefined): string {
  if (!settings) return ''
  if (settings.kind === 'drive') return settings.path
  return `${settings.user ? `${settings.user}@` : ''}${settings.host}/${settings.share}${settings.folder ? `/${settings.folder}` : ''}`
}

/**
 * A long path shortened in the middle, both ends kept: "me@192.168.1.10:445/music/Music_Sync_test" →
 * "me@192.168.1.10:445/.../Music_Sync_test" (the start up to its first separator, and the last folder). Only a
 * path without folders in between is cut by letters.
 */
export function middle(path: string, max = 32): string {
  if (path.length <= max) return path
  const sep = path.includes('\\') ? '\\' : '/'
  const parts = path.split(sep)
  if (parts.length > 2) return `${parts[0]}${sep}...${sep}${parts[parts.length - 1]}`
  const keep = Math.floor((max - 3) / 2)
  return `${path.slice(0, keep)}...${path.slice(path.length - keep)}`
}

/** On a phone: a path as its last folder only ("…/Music_Sync_test"); the details show all of it. */
export function lastPart(path: string): string {
  const sep = path.includes('\\') ? '\\' : '/'
  const parts = path.split(sep).filter(Boolean)
  return parts.length > 1 ? `…${sep}${parts[parts.length - 1]}` : path
}

/** A folder's place in words: an Android folder (a content:// address) as its path on the phone ("Music/Melue"). */
export function readablePath(location: string): string {
  if (!location.startsWith('content://')) return location
  const last = decodeURIComponent(location.split('/').pop() ?? '')
  return last.includes(':') ? last.slice(last.indexOf(':') + 1) || last : last
}

const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString()

// The time "12 min ago" counts from: one clock for the app, moving on every 30 s and as soon as the app is looked at
// again (after hours in the background it would say "15 min ago" still). Read in a computed or a template, the words
// keep up by themselves.
const clockNow = ref(Date.now())
let ticking = false
function currentTime(): number {
  if (!ticking && typeof window !== 'undefined') {
    ticking = true
    const tick = () => (clockNow.value = Date.now())
    setInterval(tick, 30_000)
    document.addEventListener('visibilitychange', tick)
    window.addEventListener('focus', tick)
  }
  return clockNow.value
}

/** "just now", "12 min ago", "at 10:42", "yesterday at 21:05", "3 Oct". */
export function ago(ms: number, now = currentTime()): string {
  const minutes = Math.floor((now - ms) / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes} min ago`
  const then = new Date(ms)
  if (sameDay(then, new Date(now))) return `at ${clock(ms)}`
  if (sameDay(then, new Date(now - 86_400_000))) return `yesterday at ${clock(ms)}`
  return then.toLocaleDateString([], { day: 'numeric', month: 'short' })
}

/** When something happened, for a list: "10:42", "Yesterday", "3 Oct". */
export function when(ms: number, now = Date.now()): string {
  const then = new Date(ms)
  if (sameDay(then, new Date(now))) return clock(ms)
  if (sameDay(then, new Date(now - 86_400_000))) return 'Yesterday'
  return then.toLocaleDateString([], { day: 'numeric', month: 'short' })
}

/** A clock time: "10:45". */
export const at = clock

/** "45 MB", "1.2 GB". */
export function size(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  return `${Math.round(bytes / 1e6)} MB`
}

/** What a sync did: "3 copied, 1 moved", or "nothing new". */
export function didWords(result: SyncResult): string {
  const parts = [
    result.copied && `${result.copied} copied`,
    result.moved && `${result.moved} moved`,
    result.removed && `${result.removed} removed`,
  ].filter(Boolean)
  if (result.cancelled) parts.push('cancelled')
  return parts.length ? parts.join(', ') : 'nothing new'
}

/** What started a sync. */
export const TRIGGER: Record<SyncTrigger, string> = {
  start: 'Start',
  return: 'Coming back',
  changed: 'The share changed',
  periodic: 'Every so often',
  button: 'Sync now',
  retry: 'Tried again',
  network: 'Network changed',
}
