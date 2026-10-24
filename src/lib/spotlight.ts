/**
 * A song pointed out on the page a note opened (tapping a toast: "Added … to …", "… plays next"): the list showing it
 * scrolls to it and lights it up a moment (SongList). `pick` finds it among the list's songs (a playlist's entry, the
 * one just added to Up next); `delay` waits for the page to come in (Up next opens and scrolls to itself first). A
 * song not found within a few seconds isn't pointed out any more.
 */
import { shallowRef } from 'vue'
import type { Song } from '@/components/songs'

export type Spotlight = { pick: (songs: Song[]) => Song | undefined; delay: number; until: number }

export const spotlight = shallowRef<Spotlight | null>(null)

const WAITS_MS = 4000

export function pointOut(pick: Spotlight['pick'], delay = 450) {
  spotlight.value = { pick, delay, until: Date.now() + WAITS_MS }
}
