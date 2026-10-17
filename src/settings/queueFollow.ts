/**
 * The queue following its list, in words and as an example: the summary under its switch (what the queue does, for
 * each combination of the choices), and "Show me how it works" (TryIt.vue): a playlist changing step by step, with
 * the queue worked out again from the start for the current choices, so changing one shows its effect at once.
 */
import type { QueueFollow } from '@/library'

/** The playing song when it leaves the list: with Keep, it finishes (QueueSettings saves it so). */
const playingOf = (f: QueueFollow) => (f.removedItems === 'keep' ? 'finish' : f.removedCurrent)

export function followSummary(f: QueueFollow): string {
  if (!f.enabled) return 'Off: the queue stays as it was started'
  const finishes = playingOf(f) === 'finish'
  const n = f.newItems
  if (f.removedItems === 'remove' && f.order === 'playlist') {
    if (finishes) {
      return {
        inPlace: 'Matches the list as it changes. The song playing now always finishes.',
        end: 'Matches the list, but new songs go at the end. The song playing now finishes.',
        next: 'Matches the list, but new songs play next. The song playing now finishes.',
      }[n]
    }
    return {
      inPlace: 'Matches the list exactly, even mid-song: a song that leaves is skipped right away.',
      end: 'Matches the list, but new songs go at the end. A song that leaves is skipped, even mid-song.',
      next: 'Matches the list, but new songs play next. A song that leaves is skipped, even mid-song.',
    }[n]
  }
  if (f.removedItems === 'remove') {
    const added = { inPlace: 'new songs near their place', end: 'new songs at the end', next: 'new songs play next' }[n]
    return `Takes the list’s changes but keeps its own order: ${added}, ${finishes ? 'the song playing finishes' : 'songs that leave are skipped'}.`
  }
  if (f.order === 'playlist') {
    return `Only gains songs, in the list’s order: ${{ inPlace: 'new ones join at their place', end: 'new ones join at the end', next: 'new ones play next' }[n]}, nothing is removed.`
  }
  return `Only gains songs: ${{ inPlace: 'new ones join near their place', end: 'new ones join at the end', next: 'new ones play next' }[n]}; nothing leaves or moves.`
}

// ---- The example

const START = ['Waterloo', 'Dancing Queen', 'Take On Me', 'Africa', 'The Sign']
export const NEW_SONG = 'Video Killed The Radio Star'
export const STEPS = [
  'The queue was started from the playlist. Waterloo has played; Dancing Queen is playing.',
  `“${NEW_SONG}” is added to the playlist, after Take On Me.`,
  '“Africa” (coming up later) is taken out of the playlist.',
  '“Dancing Queen” (playing now) is taken out of the playlist.',
  'The playlist is sorted A–Z.',
]

export interface QueueEntry {
  title: string
  playing: boolean
  /** What happened to it, shown as a small label. */
  note?: 'new' | 'removed' | 'skipped' | 'finishes, then leaves' | 'not in the list'
  /** Taken out at this step: shown struck through, then gone at the next. */
  gone?: boolean
}

interface Item { title: string; removedAt?: number; skipped?: boolean; finishing?: boolean; kept?: boolean; added?: boolean }

/** The playlist and the queue after `step` changes, for these choices. */
export function simulate(f: QueueFollow, step: number): { list: string[]; queue: QueueEntry[] } {
  let list = [...START]
  let queue: Item[] = START.map((title) => ({ title }))
  let current = 1
  const playing = playingOf(f)
  const at = (title: string) => queue.findIndex((x) => x.title === title && x.removedAt === undefined)
  const leave = (title: string, now: number) => {
    list = list.filter((x) => x !== title)
    if (!f.enabled) return
    const i = at(title)
    if (i < 0) return
    if (i === current) {
      if (playing === 'skip') {
        Object.assign(queue[i], { removedAt: now, skipped: true })
        current += 1
      } else if (f.removedItems === 'remove') queue[i].finishing = true
      else queue[i].kept = true
    } else if (f.removedItems === 'remove') queue[i].removedAt = now
    else queue[i].kept = true
  }
  for (let now = 1; now <= step; now++) {
    if (now === 1) {
      list.splice(list.indexOf('Take On Me') + 1, 0, NEW_SONG)
      if (!f.enabled) continue
      const item: Item = { title: NEW_SONG, added: true }
      if (f.newItems === 'next') queue.splice(current + 1, 0, item)
      else if (f.newItems === 'end') queue.push(item)
      else queue.splice(Math.max(at(list[list.indexOf(NEW_SONG) - 1]), current) + 1, 0, item)
    }
    if (now === 2) leave('Africa', now)
    if (now === 3) leave('Dancing Queen', now)
    if (now === 4) {
      list.sort((a, b) => a.localeCompare(b))
      if (!f.enabled || f.order !== 'playlist') continue
      const upcoming = queue.slice(current + 1)
      const rank = (x: Item) => (list.includes(x.title) ? list.indexOf(x.title) : list.length)
      queue = [
        ...queue.slice(0, current + 1),
        ...upcoming.filter((x) => x.removedAt === undefined).sort((a, b) => rank(a) - rank(b)),
        ...upcoming.filter((x) => x.removedAt !== undefined),
      ]
    }
  }
  const entries = queue
    .map((x, i) => ({ x, i }))
    .filter(({ x }) => x.removedAt === undefined || x.removedAt === step)
    .map(({ x, i }): QueueEntry => ({
      title: x.title,
      playing: i === current,
      gone: x.removedAt !== undefined,
      note: x.skipped ? 'skipped' : x.removedAt !== undefined ? 'removed' : x.finishing ? 'finishes, then leaves' : x.kept ? 'not in the list' : x.added ? 'new' : undefined,
    }))
  return { list, queue: entries }
}
