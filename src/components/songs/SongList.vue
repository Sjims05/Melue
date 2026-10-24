<script setup lang="ts">
/**
 * A list of songs (SongRow). `queue`: drag handles, numbers, ✕ and reordering by dragging a handle (picked up, the
 * others make room, it glides into place). `az`: grouped under letter headings (A, B, C… by title; # for the rest). `removable`: ✕ (a
 * playlist). The song playing is highlighted with its bars; `playsNext` (the queue) marks the song right after it
 * ("Plays next"; the first, in a list without the playing song: Up next). While a song is dragged, the numbers and "Plays next" follow the order it would make.
 *
 * Narrow (a phone): the album goes, then the length (container widths, not the window's).
 */
import { computed, nextTick, ref, watch } from 'vue'
import { spotlight } from '@/lib/spotlight'
import { letterOf } from './letters'
import SongRow from './SongRow.vue'
import type { Song, SongAction, SongMenuPlace } from './songMenu'

const props = defineProps<{
  songs: Song[]
  queue?: boolean
  az?: boolean
  removable?: boolean
  /** A playlist's songs: drag handles, numbers and ✕ like the queue (without its strong highlight). */
  reorderable?: boolean
  /** Songs that stay when taken out (a linked playlist's: they turn greyed, "Removed here"): no folding away. */
  keepOnRemove?: (song: Song) => boolean
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
  /** Each song's heading ("Disc 2"); one shows before the first song of each. */
  headingOf?: (song: Song) => string
  /** A number before each song (an album's track numbers); the queue numbers its own. */
  numberOf?: (song: Song, index: number) => number | string | undefined
  /** No covers and no album column on the rows (an album's songs: they'd be the same on each). */
  albumPage?: boolean
}>()
const emit = defineEmits<{
  /** `via`: its play button, or a double-click on the row. */
  play: [song: Song, via: 'button' | 'doubleClick']
  pause: []
  like: [song: Song, liked: boolean]
  remove: [song: Song, index: number]
  /** Songs start folding away (taken out, or Clear): how many, so what comes after can make room as they close. */
  folding: [count: number]
  move: [from: number, to: number]
  action: [song: Song, action: SongAction]
}>()

const rows = computed(() =>
  props.songs.map((song, index) => ({
    song,
    index,
    heading: props.headingOf
      ? index === 0 || props.headingOf(song) !== props.headingOf(props.songs[index - 1])
        ? props.headingOf(song)
        : null
      : props.az && (index === 0 || letterOf(song.title) !== letterOf(props.songs[index - 1].title))
        ? letterOf(song.title)
        : null,
  })),
)

// ---- Reordering (the queue), with the pointer (a mouse, a finger), not the browser's drag and drop: on Windows,
// Tauri takes drags for files dropped on Melue. Picked up by its handle, the song grows a little and lifts (a shadow)
// and follows the pointer; the songs it passes slide aside to make room; let go, it glides into its place and settles.
const LIFT = 1.03
const drag = ref<{ from: number; to: number; startY: number; dy: number; centers: number[]; height: number } | null>(null)
// After letting go, the rows take their new places without sliding back.
const settling = ref(false)
// The song picked up (its key), highlighted until it has glided into its place; then its highlight's background fades
// away (`fading`; its words and icons change at once: fading those too passes through grey on grey).
const lifted = ref<Song['id'] | null>(null)
const fading = ref<Song['id'] | null>(null)
function putDown() {
  fading.value = lifted.value
  lifted.value = null
  setTimeout(() => (fading.value = null), 320)
}
const list = ref<HTMLElement>()

function grab(event: PointerEvent, index: number) {
  if (!(props.queue || props.reorderable) || props.songs[index].later || !(event.target as HTMLElement).closest('.song-handle')) return
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
    row.classList.add('z-10', 'bg-card', 'shadow-lg')
    void row.offsetHeight
    row.style.transition = 'translate 260ms cubic-bezier(.2,.8,.2,1), scale 260ms cubic-bezier(.2,.8,.2,1), box-shadow 260ms ease-out'
    row.style.translate = ''
    row.style.scale = ''
    row.classList.remove('shadow-lg')
    setTimeout(() => {
      row.style.transition = ''
      row.classList.remove('z-10', 'bg-card')
      putDown()
    }, 280)
  } else putDown()
  requestAnimationFrame(() => requestAnimationFrame(() => (settling.value = false)))
}
// ---- Moving with the keyboard (Shift+Enter on a song): picked up (the dragged look), ↑ / ↓ move it a place at a time,
// the others making room; Enter (or Shift+Enter) puts it down, gliding into place; Esc puts it back. While it's picked
// up, Enter only puts it down (it never plays), and the list keeps the arrows (data-moving: lib/arrowNav.ts).
const keyMoving = ref(false)
function lastPlace() {
  const later = props.songs.findIndex((s) => s.later)
  return (later < 0 ? props.songs.length : later) - 1
}
function moveKey(e: KeyboardEvent, index: number) {
  if (!(props.queue || props.reorderable) || e.ctrlKey || e.altKey || e.metaKey) return
  const d = drag.value
  if (!keyMoving.value || !d) {
    if (e.key === 'Enter' && e.shiftKey && e.target === e.currentTarget && !props.songs[index].later && !drag.value) {
      e.preventDefault()
      const row = e.currentTarget as HTMLElement
      drag.value = { from: index, to: index, startY: 0, dy: 0, centers: [], height: row.offsetHeight }
      lifted.value = props.songs[index].key ?? props.songs[index].id
      keyMoving.value = true
    }
    return
  }
  e.preventDefault()
  e.stopPropagation()
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    d.to = Math.min(lastPlace(), Math.max(0, d.to + (e.key === 'ArrowDown' ? 1 : -1)))
    d.dy = (d.to - d.from) * d.height
    const row = e.currentTarget as HTMLElement
    // In view as it goes (once it has moved there).
    setTimeout(() => row.scrollIntoView({ block: 'nearest', behavior: 'smooth' }), 200)
  } else if (e.key === 'Enter' || e.key === 'Escape') {
    if (e.key === 'Escape') {
      d.to = d.from
      d.dy = 0
    }
    putDownKeys()
  }
}
async function putDownKeys() {
  const d = drag.value
  if (!d) return
  const key = String(props.songs[d.from].key ?? props.songs[d.from].id)
  keyMoving.value = false
  await letGo()
  // Moved, its row was put elsewhere in the page: the keyboard stays on it.
  await nextTick()
  list.value?.querySelector<HTMLElement>(`:scope > [data-song="${CSS.escape(key)}"]`)?.focus({ preventScroll: true })
}

// ---- Taking songs out (✕, Clear): they fold away (height and look), the songs under them moving up, and only then
// are they taken out. Each row's fold is undone once the list has changed (a row can be kept and show another song).
const FOLD = { duration: 260, easing: 'cubic-bezier(.2,.8,.2,1)' }
// The songs folding away here: when they leave the list, they've folded already.
const foldingHere = new Set<string>()
async function foldAway(keys: Song['id'][]) {
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return () => {}
  keys.forEach((key) => foldingHere.add(String(key)))
  const rows = keys
    .map((key) => list.value?.querySelector<HTMLElement>(`:scope > [data-song="${CSS.escape(String(key))}"]`))
    .filter((row): row is HTMLElement => !!row)
  emit('folding', rows.length)
  const folds = rows.map((row) => {
    row.style.overflow = 'hidden'
    return row.animate([{ height: `${row.offsetHeight}px`, opacity: 1 }, { height: '0px', opacity: 0 }], { ...FOLD, fill: 'forwards' })
  })
  await Promise.all(folds.map((f) => f.finished.catch(() => {})))
  return () => {
    keys.forEach((key) => foldingHere.delete(String(key)))
    folds.forEach((f) => f.cancel())
    rows.forEach((row) => (row.style.overflow = ''))
  }
}
async function remove(song: Song, index: number) {
  if (props.keepOnRemove?.(song)) return emit('remove', song, index)
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

// ---- The queue or a playlist changing: songs coming in (Play next, Add to queue, from another device) unfold into
// place; songs taken out elsewhere (another device) fold away where they were (a copy of their row, as the row
// itself is gone); songs that moved without being dragged here (on the other device, the queue going on to the next
// song) glide from where they were to their new places, the others making way. Not the rows shown ahead (after the
// queue starts over), and not while a song is dragged or just let go (that glides on its own).
const keyOf = (s: Song) => String(s.key ?? s.id)
// Gentler than FOLD (which answers a tap at once): a change from elsewhere starts and ends softly, so it's seen.
const ARRIVE = { duration: 320, easing: 'cubic-bezier(.4,0,.2,1)' }
// Started in the next frame, holding its first look until then: the frame that changes the list is a long one (the
// new rows drawn), and an animation started in it counts from that frame's start, so most of its way would be
// skipped at once.
function animateNextFrame(el: HTMLElement, keyframes: Record<string, string | number>[], options: KeyframeAnimationOptions) {
  const first = keyframes[0]
  for (const [property, value] of Object.entries(first)) el.style.setProperty(property, String(value))
  return new Promise<void>((done) =>
    requestAnimationFrame(() => {
      const animation = el.animate(keyframes, options)
      for (const property of Object.keys(first)) el.style.removeProperty(property)
      animation.finished.catch(() => {}).then(() => done())
    }),
  )
}
const rowOf = (key: string) => list.value?.querySelector<HTMLElement>(`:scope > [data-song="${CSS.escape(key)}"]`)
watch(
  () => props.songs.filter((s) => !s.later).map(keyOf),
  async (now, before) => {
    if (!(props.queue || props.reorderable) || !before || matchMedia('(prefers-reduced-motion: reduce)').matches) return
    if (drag.value || settling.value) return
    // Where each row is now in the list, before it changes (this runs before the page is drawn again): in the list, not
    // on the screen, so the page scrolling or sliding in doesn't count as songs moving. Not on screen (its page away):
    // nothing to animate from.
    if (!list.value?.offsetParent) return
    const listTop = (el: HTMLElement | null | undefined) => el && list.value ? el.getBoundingClientRect().top - list.value.getBoundingClientRect().top : undefined
    const was = new Map(before.map((key) => [key, listTop(rowOf(key))]))
    const had = new Set(before)
    const added = now.filter((key) => !had.has(key))
    // Taken out elsewhere: a copy of each row, with the song after it that stays (it goes back in before that one).
    const staying = new Set(now)
    const gone = before
      .map((key, i) => ({ key, row: rowOf(key), next: before.slice(i + 1).find((k) => staying.has(k)) }))
      .filter((g) => !staying.has(g.key) && !foldingHere.has(g.key) && g.row)
      .map((g) => ({ copy: g.row!.cloneNode(true) as HTMLElement, next: g.next }))
    // Until the copies are in, the list keeps its height: at the bottom of the page, a row taken out would make the
    // page shorter and its scroll jump up at once (before the copy could fold).
    if (gone.length && list.value) list.value.style.minHeight = `${list.value.offsetHeight}px`
    await nextTick()
    // A whole new queue (another list played) or list: not song by song.
    if (added.length > 20 || gone.length > 20) {
      if (list.value) list.value.style.minHeight = ''
      return
    }
    for (const { copy, next } of gone) {
      copy.removeAttribute('data-song')
      copy.setAttribute('role', 'presentation')
      copy.setAttribute('aria-hidden', 'true')
      copy.style.pointerEvents = 'none'
      copy.style.overflow = 'hidden'
      const before = next ? rowOf(next) : null
      list.value?.insertBefore(copy, before ?? null)
      const height = copy.offsetHeight
      animateNextFrame(copy, [{ height: `${height}px`, opacity: 1 }, { height: '0px', opacity: 0 }], { ...ARRIVE, fill: 'forwards' }).then(() =>
        copy.remove(),
      )
    }
    if (list.value) list.value.style.minHeight = ''
    for (const key of now) {
      const row = rowOf(key)
      if (!row) continue
      if (!had.has(key)) {
        row.style.overflow = 'hidden'
        const height = row.offsetHeight
        animateNextFrame(row, [{ height: '0px', opacity: 0 }, { height: `${height}px`, opacity: 1 }], ARRIVE).then(() => (row.style.overflow = ''))
        continue
      }
      const from = was.get(key)
      const offset = from == null ? 0 : from - (listTop(row) ?? from)
      if (Math.abs(offset) > 1) animateNextFrame(row, [{ translate: `0 ${offset}px` }, { translate: '0 0' }], ARRIVE)
    }
  },
)

// ---- A song pointed out (a toast tapped: lib/spotlight.ts): once it's in this list, scrolled to the middle and lit up a
// moment (the look of a song being moved), then fading.
watch(
  [spotlight, () => props.songs],
  async ([wanted]) => {
    if (!wanted) return
    if (Date.now() > wanted.until) return void (spotlight.value = null)
    const song = wanted.pick(props.songs)
    if (!song) return
    spotlight.value = null
    const key = song.key ?? song.id
    await new Promise((done) => setTimeout(done, wanted.delay))
    const row = rowOf(String(key))
    if (!row) return
    row.scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block: 'center' })
    lifted.value = key
    setTimeout(() => lifted.value === key && putDown(), 1500)
  },
  { immediate: true },
)

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

// The keyboard: one row at a time takes Tab (the one last moved to); the keys within: lib/hotkeys.ts (listKey).
const focused = ref<number | null>(null)
</script>

<template>
  <div ref="list" class="@container" :data-moving="keyMoving || undefined" :class="{ 'song-on-card': onCard, 'song-strong': queue }" role="list">
    <template v-for="{ song, index, heading } in rows" :key="song.key ?? song.id">
      <div v-if="heading" class="mt-4 mb-1 px-2 text-sm font-semibold text-muted-foreground first:mt-0">{{ heading }}</div>
      <!-- While a song is picked up and placed, the others don't light up under the pointer (song-quiet: their hover
           look is off, the pointer still reaches them, so the one under it lights up as soon as placing is done). -->
      <div
        role="listitem"
        class="relative rounded-lg outline-none"
        :class="[
          lifted !== null && lifted !== (song.key ?? song.id) && 'song-quiet',
          drag?.from === index
            ? [
                'z-10 bg-card shadow-lg duration-200 ease-[cubic-bezier(.2,.8,.2,1)]',
                // Moved with the keys: it glides a place at a time (with the pointer, it follows it).
                keyMoving ? 'transition-[scale,box-shadow,translate]' : 'transition-[scale,box-shadow]',
              ]
            : !settling && 'transition-[translate] duration-200 ease-[cubic-bezier(.2,.8,.2,1)] motion-reduce:transition-none',
        ]"
        :style="{ translate: moved(index), scale: drag?.from === index ? LIFT : undefined }"
        :data-song="song.key ?? song.id"
        :tabindex="index === (focused ?? 0) ? 0 : -1"
        @focus="focused = index"
        @keydown="moveKey($event, index)"
        @pointerdown="grab($event, index)"
        @pointermove="follow"
        @pointerup="letGo"
        @pointercancel="letGo"
      >
        <SongRow
          :song="song"
          :number="queue || reorderable ? placeOf(index) + 1 : numberOf?.(song, index)"
          :no-cover="albumPage"
          :no-album="albumPage"
          :handle="queue || reorderable"
          :removable="queue || reorderable || removable"
          :current="song.id === currentId"
          :lifted="lifted === (song.key ?? song.id)"
          :fading="fading === (song.key ?? song.id)"
          :playing="playing"
          :note="song.later && !songs[index - 1]?.later ? song.later : (song.note ?? nextNote(index))"
          :unavailable="unavailable?.includes(song.id)"
          :local-only="localOnly?.includes(song.id)"
          :place="place"
          @play="(via) => emit('play', song, via)"
          @pause="emit('pause')"
          @like="(liked) => emit('like', song, liked)"
          @remove="remove(song, index)"
          @action="(action) => emit('action', song, action)"
        />
      </div>
    </template>
  </div>
</template>
