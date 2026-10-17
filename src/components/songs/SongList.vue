<script setup lang="ts">
/**
 * A list of songs (SongRow). `queue`: drag handles, numbers, ✕ and reordering by dragging a handle (picked up, the
 * others make room, it glides into place). `az`: grouped under letter headings (A, B, C… by title; # for the rest). `removable`: ✕ (a
 * playlist). The song playing is highlighted with its bars; `playsNext` (the queue) marks the song right after it
 * ("Plays next"; the first, in a list without the playing song: Up next). While a song is dragged, the numbers and "Plays next" follow the order it would make.
 *
 * Narrow (a phone): the album goes, then the length (container widths, not the window's).
 */
import { computed, nextTick, ref } from 'vue'
import SongRow from './SongRow.vue'
import type { Song, SongAction, SongMenuPlace } from './songMenu'

const props = defineProps<{
  songs: Song[]
  queue?: boolean
  az?: boolean
  removable?: boolean
  /** The song playing (or paused on). */
  currentId?: Song['id'] | null
  playing?: boolean
  /** The queue: "Plays next" on the song right after the one playing. */
  playsNext?: boolean
  /** Songs that can't play (missing). */
  unavailable?: Song['id'][]
  /** Songs only on this device (not on the share): the "This device" badge. */
  localOnly?: Song['id'][]
  place?: SongMenuPlace
  /** The list sits on a card (a panel, a dialog): its highlight is grey in light mode (white wouldn't show). */
  onCard?: boolean
}>()
const emit = defineEmits<{
  play: [song: Song]
  pause: []
  like: [song: Song, liked: boolean]
  remove: [song: Song, index: number]
  move: [from: number, to: number]
  action: [song: Song, action: SongAction]
}>()

/** The first letter of a title, for the A–Z headings. */
const letter = (title: string) => {
  const first = title.trim().replace(/^the\s+/i, '').charAt(0).toUpperCase()
  return /[A-Z]/.test(first) ? first : '#'
}
const rows = computed(() =>
  props.songs.map((song, index) => ({
    song,
    index,
    heading: props.az && (index === 0 || letter(song.title) !== letter(props.songs[index - 1].title)) ? letter(song.title) : null,
  })),
)

// ---- Reordering (the queue), with the pointer (a mouse, a finger), not the browser's drag and drop: on Windows,
// Tauri takes drags for files dropped on Melue. Picked up by its handle, the song grows a little and lifts (a shadow)
// and follows the pointer; the songs it passes slide aside to make room; let go, it glides into its place and settles.
const LIFT = 1.03
const drag = ref<{ from: number; to: number; startY: number; dy: number; centers: number[]; height: number } | null>(null)
// After letting go, the rows take their new places without sliding back.
const settling = ref(false)
// The song picked up (its key), highlighted until it has glided into its place; then its highlight fades (`fading`).
const lifted = ref<Song['id'] | null>(null)
const fading = ref<Song['id'] | null>(null)
function putDown() {
  fading.value = lifted.value
  lifted.value = null
  setTimeout(() => (fading.value = null), 320)
}
const list = ref<HTMLElement>()

function grab(event: PointerEvent, index: number) {
  if (!props.queue || props.songs[index].later || !(event.target as HTMLElement).closest('.song-handle')) return
  event.preventDefault()
  const wrapper = event.currentTarget as HTMLElement
  const rows = [...list.value!.querySelectorAll<HTMLElement>(':scope > [role=listitem]')]
  wrapper.setPointerCapture(event.pointerId)
  lifted.value = props.songs[index].key ?? props.songs[index].id
  drag.value = {
    from: index,
    to: index,
    startY: event.clientY,
    dy: 0,
    centers: rows.map((r) => r.getBoundingClientRect().top + r.offsetHeight / 2),
    height: wrapper.offsetHeight,
  }
}
function follow(event: PointerEvent) {
  const d = drag.value
  if (!d) return
  d.dy = event.clientY - d.startY
  const center = d.centers[d.from] + d.dy
  // Its new place: after every other song whose middle it has passed.
  // Not among the songs shown ahead (later): those come after the queue.
  const last = props.songs.findIndex((s) => s.later)
  d.to = Math.min(d.centers.filter((c, i) => i !== d.from && c < center).length, last < 0 ? Infinity : last - 1)
}
async function letGo() {
  const d = drag.value
  if (!d) return
  const id = props.songs[d.from].key ?? props.songs[d.from].id
  const rowOf = () => list.value?.querySelector<HTMLElement>(`:scope > [data-song="${CSS.escape(String(id))}"]`)
  const was = rowOf()?.getBoundingClientRect().top ?? 0
  settling.value = true
  drag.value = null
  if (d.to !== d.from) emit('move', d.from, d.to)
  await nextTick()
  // Glide: drawn where it was let go (still lifted), then to its place, settling down.
  const row = rowOf()
  if (row) {
    const offset = was - row.getBoundingClientRect().top
    row.style.transition = 'none'
    row.style.translate = `0 ${offset}px`
    row.style.scale = String(LIFT)
    row.classList.add('z-10', 'shadow-lg')
    void row.offsetHeight
    row.style.transition = 'translate 260ms cubic-bezier(.2,.8,.2,1), scale 260ms cubic-bezier(.2,.8,.2,1), box-shadow 260ms ease-out'
    row.style.translate = ''
    row.style.scale = ''
    row.classList.remove('shadow-lg')
    setTimeout(() => {
      row.style.transition = ''
      row.classList.remove('z-10')
      putDown()
    }, 280)
  } else putDown()
  requestAnimationFrame(() => requestAnimationFrame(() => (settling.value = false)))
}
// ---- Taking songs out (✕, Clear): they fold away (height and look), the songs under them moving up, and only then
// are they taken out. Each row's fold is undone once the list has changed (a row can be kept and show another song).
const FOLD = { duration: 260, easing: 'cubic-bezier(.2,.8,.2,1)' }
async function foldAway(keys: Song['id'][]) {
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return () => {}
  const rows = keys
    .map((key) => list.value?.querySelector<HTMLElement>(`:scope > [data-song="${CSS.escape(String(key))}"]`))
    .filter((row): row is HTMLElement => !!row)
  const folds = rows.map((row) => {
    row.style.overflow = 'hidden'
    return row.animate([{ height: `${row.offsetHeight}px`, opacity: 1 }, { height: '0px', opacity: 0 }], { ...FOLD, fill: 'forwards' })
  })
  await Promise.all(folds.map((f) => f.finished.catch(() => {})))
  return () => {
    folds.forEach((f) => f.cancel())
    rows.forEach((row) => (row.style.overflow = ''))
  }
}
async function remove(song: Song, index: number) {
  const undo = await foldAway([song.key ?? song.id])
  emit('remove', song, index)
  await nextTick()
  undo()
}
/** Clear (the queue's): the songs fold away together; then `done` takes them out. */
async function foldAwayAll(songs: Song[], done: () => unknown) {
  const undo = await foldAway(songs.map((s) => s.key ?? s.id))
  await done()
  await nextTick()
  undo()
}
defineExpose({ foldAwayAll })

/** A row's place in the order a drag would make (its place as it is, without a drag). */
function placeOf(index: number): number {
  const d = drag.value
  if (!d) return index
  if (index === d.from) return d.to
  if (d.from < index && index <= d.to) return index - 1
  if (d.to <= index && index < d.from) return index + 1
  return index
}
const currentIndex = computed(() => props.songs.findIndex((s) => s.id === props.currentId))
/** "Plays next": the song right after the one playing, in the order as it is now (also mid-drag). */
function nextNote(index: number): string | undefined {
  if (!props.playsNext) return undefined
  // Up next (the playing song isn't in the list): the first one.
  const next = currentIndex.value < 0 ? 0 : placeOf(currentIndex.value) + 1
  return placeOf(index) === next ? 'Plays next' : undefined
}

/** Where a row is drawn while a song is dragged: the song follows the pointer, the ones it passed make room. */
function moved(index: number): string | undefined {
  const d = drag.value
  if (!d) return undefined
  if (index === d.from) return `0 ${d.dy}px`
  if (d.from < index && index <= d.to) return `0 ${-d.height}px`
  if (d.to <= index && index < d.from) return `0 ${d.height}px`
  return undefined
}
</script>

<template>
  <div ref="list" class="@container" :class="{ 'song-on-card': onCard, 'song-strong': queue }" role="list">
    <template v-for="{ song, index, heading } in rows" :key="song.key ?? song.id">
      <div v-if="heading" class="mt-4 mb-1 px-2 text-sm font-semibold text-muted-foreground first:mt-0">{{ heading }}</div>
      <!-- While a song is picked up and placed, the others don't light up under the pointer: only it is highlighted (all of
           each row: its context menu trigger sets the pointer back on in its own style). -->
      <div
        role="listitem"
        class="relative rounded-lg"
        :class="[
          lifted !== null && lifted !== (song.key ?? song.id) && 'pointer-events-none [&_*]:pointer-events-none!',
          drag?.from === index
            ? 'z-10 shadow-lg transition-[scale,box-shadow] duration-200 ease-[cubic-bezier(.2,.8,.2,1)]'
            : !settling && 'transition-[translate] duration-200 ease-[cubic-bezier(.2,.8,.2,1)] motion-reduce:transition-none',
        ]"
        :style="{ translate: moved(index), scale: drag?.from === index ? LIFT : undefined }"
        :data-song="song.key ?? song.id"
        @pointerdown="grab($event, index)"
        @pointermove="follow"
        @pointerup="letGo"
        @pointercancel="letGo"
      >
        <SongRow
          :song="song"
          :number="queue ? placeOf(index) + 1 : undefined"
          :handle="queue"
          :removable="queue || removable"
          :current="song.id === currentId"
          :lifted="lifted === (song.key ?? song.id)"
          :fading="fading === (song.key ?? song.id)"
          :playing="playing"
          :note="song.later && !songs[index - 1]?.later ? song.later : nextNote(index)"
          :unavailable="unavailable?.includes(song.id)"
          :local-only="localOnly?.includes(song.id)"
          :place="place"
          @play="emit('play', song)"
          @pause="emit('pause')"
          @like="(liked) => emit('like', song, liked)"
          @remove="remove(song, index)"
          @action="(action) => emit('action', song, action)"
        />
      </div>
    </template>
  </div>
</template>
