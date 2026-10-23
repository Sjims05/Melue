/**
 * The playing song's lyrics (src-tauri library/lyrics.rs: a .lrc next to it, or its tags), read when the song changes,
 * and a smooth clock for following them: the player's position, moved on every frame while lyrics are shown (the
 * player itself only says where it is now and then).
 */
import { invoke } from '@tauri-apps/api/core'
import { effectScope, onActivated, onBeforeUnmount, onDeactivated, ref, shallowRef, watch } from 'vue'
import { current, nowPlaying } from './nowPlaying'

export interface Lyrics {
  /** Its lines have times (they follow the song). */
  synced: boolean
  lines: { atMs: number | null; text: string; words: { atMs: number; text: string }[] }[]
  /** "file" (a .lrc), "tags", or "online" (LRCLIB). */
  source: 'file' | 'tags' | 'online'
  /** The song has no words (LRCLIB says so). */
  instrumental: boolean
  /** Moved by hand for this song (ms; positive: later). The line and word times don't include it. */
  offsetMs: number
  /** Lines tapped as they started being sung ("Sync by tapping"): the line, and when (ms into the song). */
  anchors: { line: number; atMs: number }[]
  /** The times are a rough guess (lyrics without any): spread over the song by syllables. */
  estimated: boolean
}

export type LyricLine = Lyrics['lines'][number]

/**
 * The lines with the song's timing set by hand: moved earlier or later (`offsetMs`), and the tapped lines where they
 * were tapped. A tapped line takes the lines after it along (up to the next tapped one): the same distance for timed
 * lyrics (a longer version's added break), stretched between two taps for estimated ones (the guess fitted to them).
 */
export function timedLines(l: Lyrics): LyricLine[] {
  const base = l.lines.map((line) => (line.atMs == null ? null : line.atMs + l.offsetMs))
  const anchors = l.anchors.filter((a) => base[a.line] != null).sort((a, b) => a.line - b.line)
  return l.lines.map((line, i) => {
    const at = base[i]
    if (at == null) return line
    const before = anchors.filter((a) => a.line <= i).pop()
    let move = (t: number) => t
    if (before) {
      const from = base[before.line]!
      const after = anchors.find((a) => a.line > i)
      if (l.estimated && after && base[after.line]! > from) {
        const stretch = (after.atMs - before.atMs) / (base[after.line]! - from)
        move = (t) => before.atMs + (t - from) * stretch
      } else move = (t) => before.atMs + (t - from)
    }
    return { ...line, atMs: Math.round(move(at)), words: line.words.map((w) => ({ ...w, atMs: Math.round(move(w.atMs + l.offsetMs)) })) }
  })
}

/** The playing song's lyrics: undefined while they're read, null when it has none. */
export const lyrics = shallowRef<Lyrics | null | undefined>(undefined)

const read = new Map<number, Lyrics | null>()
let started = false

/**
 * Once (Now playing): reads the lyrics of each song that plays. The app's, not the page's: on a phone Now playing is
 * taken away when it closes, and a watch of its own would stop with it (the lyrics stuck on that song).
 */
export function followLyrics() {
  if (started) return
  started = true
  effectScope(true).run(() => watch(
    () => Number(current.value?.id) || null,
    async (id) => {
      if (!id) return void (lyrics.value = null)
      if (read.has(id)) return void (lyrics.value = read.get(id))
      lyrics.value = undefined
      const found = await invoke<Lyrics | null>('library_lyrics', { id }).catch(() => null)
      // None yet isn't kept: on Wi-Fi later they may be found.
      if (found) read.set(id, found)
      if (Number(current.value?.id) === id) lyrics.value = found
    },
    { immediate: true },
  ))
}

/**
 * "Try another version" (lyrics from LRCLIB that don't fit the song): the next of LRCLIB's entries. Resolves false when
 * there's no other (or LRCLIB can't be reached).
 */
export async function tryOtherVersion(): Promise<boolean> {
  const id = Number(current.value?.id)
  if (!id) return false
  const other = await invoke<Lyrics | null>('library_lyrics_other_version', { id }).catch(() => null)
  if (!other) return false
  read.set(id, other)
  if (Number(current.value?.id) === id) lyrics.value = other
  return true
}

let saving: ReturnType<typeof setTimeout> | undefined
/** Changes the playing song's lyrics timing (kept for that song, and sent to the other devices). */
function setTiming(change: Partial<Pick<Lyrics, 'offsetMs' | 'anchors'>>) {
  const id = Number(current.value?.id)
  const now = lyrics.value
  if (!id || !now) return
  const next = { ...now, ...change }
  lyrics.value = next
  read.set(id, next)
  // Kept a moment after the last press (a few presses in a row are one change).
  clearTimeout(saving)
  saving = setTimeout(() => invoke('library_set_lyrics_timing', { id, offsetMs: next.offsetMs, anchors: next.anchors }).catch(() => {}), 600)
}

/** Moves the playing song's lyrics earlier or later (ms; positive: later). */
export const setLyricsOffset = (offsetMs: number) => setTiming({ offsetMs })

/** "Sync by tapping": line `line` starts being sung at `atMs` (a tap on it again moves it). */
export function tapLine(line: number, atMs: number) {
  const anchors = (lyrics.value?.anchors ?? []).filter((a) => a.line !== line)
  setTiming({ anchors: [...anchors, { line, atMs: Math.round(atMs) }].sort((a, b) => a.line - b.line) })
}

/** Forgets the taps. */
export const clearTaps = () => setTiming({ anchors: [] })

/** Forgets one line's tap. */
export const removeTap = (line: number) => setTiming({ anchors: (lyrics.value?.anchors ?? []).filter((a) => a.line !== line) })

/** Moves one line's tap a little (a tap is rarely spot on): `byMs` later (negative: earlier). */
export function nudgeTap(line: number, byMs: number) {
  const anchors = (lyrics.value?.anchors ?? []).map((a) => (a.line === line ? { ...a, atMs: Math.max(0, a.atMs + byMs) } : a))
  setTiming({ anchors })
}

/** Milliseconds into the song, every frame while the caller is shown (not while its page is kept aside). */
export function useLyricsClock() {
  const ms = ref(0)
  let frame = 0
  const step = () => {
    const s = nowPlaying.state
    if (s && !document.hidden) {
      const moved = s.isPlaying ? Math.min(5000, performance.now() - nowPlaying.stateAt) : 0
      ms.value = Math.min((s.duration || Infinity) * 1000, s.currentTime * 1000 + moved)
    }
    frame = requestAnimationFrame(step)
  }
  const start = () => (cancelAnimationFrame(frame), (frame = requestAnimationFrame(step)))
  const stop = () => cancelAnimationFrame(frame)
  start()
  onActivated(start)
  onDeactivated(stop)
  onBeforeUnmount(stop)
  return ms
}
