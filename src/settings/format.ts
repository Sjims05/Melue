/** How the settings put things into words: the share's name, times, sizes, what a sync did. */
import type { ShareSettings, SyncResult, SyncTrigger } from '@/library/sync'

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

const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString()

/** "just now", "12 min ago", "at 10:42", "yesterday at 21:05", "3 Oct". */
export function ago(ms: number, now = Date.now()): string {
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
