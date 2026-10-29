<script setup lang="ts">
/**
 * A list of songs (SongRow). `queue`: drag handles, numbers, ✕ and reordering by dragging a handle (picked up, the
 * others make room, it glides into place). `az`: grouped under letter headings (A, B, C… by title; # for the rest). `removable`: ✕ (a
 * playlist). The song playing is highlighted with its bars; `playsNext` (the queue) marks the song right after it
 * ("Plays next"; the first, in a list without the playing song: Up next). While a song is dragged, the numbers and "Plays next" follow the order it would make.
 *
 * Narrow (a phone): the album goes, then the length (container widths, not the window's).
 *
 * Only the rows in and near the view are drawn; the rest are space of their height, so a long list scrolls and resizes
 * as a short one does (a list with headings, whose heights differ, draws all its rows).
 */
import { computed, nextTick, onActivated, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { spotlight } from '@/lib/spotlight'
import { letterOf } from './letters'
import SongRow from './SongRow.vue'
import { provideBadgeLevels } from './badgeFit'
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
// Its rows' device badges and notes: words or icons, all together (badgeFit.ts).
provideBadgeLevels()
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
// `centers`: each row's middle when it was picked up (on the screen then); `scrolled`: the list's scroll then, so a
// scroll while it's held (the wheel, or the list scrolling at its edges) counts as moving it.
const drag = ref<{ from: number; to: number; startY: number; lastY: number; scrolled: number; dy: number; centers: number[]; height: number } | null>(null)
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
  wrapper.setPointerCapture(event.pointerId)
  lifted.value = props.songs[index].key ?? props.songs[index].id
  drag.value = {
    from: index,
    to: index,
    startY: event.clientY,
    lastY: event.clientY,
    scrolled: scroller?.scrollTop ?? 0,
    dy: 0,
    centers: centers(),
    height: wrapper.offsetHeight,
  }
  requestAnimationFrame(scrollAtEdges)
}
/** Each row's middle on the screen now: worked out from the row height (not every row is drawn), or measured. */
function centers(): number[] {
  const el = list.value!
  if (windowed.value) {
    const top = el.getBoundingClientRect().top
    return props.songs.map((_, i) => top + i * rowHeight.value + rowHeight.value / 2)
  }
  return [...el.querySelectorAll<HTMLElement>(':scope > [role=listitem]')].map((r) => r.getBoundingClientRect().top + r.offsetHeight / 2)
}
function follow(event: PointerEvent) {
  const d = drag.value
  if (!d) return
  d.lastY = event.clientY
  placeHeld(d)
}
/** Where the song held goes: after every other song whose middle it has passed (the pointer and the scroll since). */
function placeHeld(d: NonNullable<typeof drag.value>) {
  d.dy = d.lastY - d.startY + ((scroller?.scrollTop ?? 0) - d.scrolled)
  const center = d.centers[d.from] + d.dy
  // Its new place: after every other song whose middle it has passed.
  // Not among the songs shown ahead (later): those come after the queue.
  const last = props.songs.findIndex((s) => s.later)
  d.to = Math.min(d.centers.filter((c, i) => i !== d.from && c < center).length, last < 0 ? Infinity : last - 1)
}
/** Held near the list's top or bottom edge: it scrolls that way, faster the nearer (to move a song past the view). */
const EDGE = 56
function scrollAtEdges() {
  const d = drag.value
  if (!d || keyMoving.value || !scroller) return
  const view = scroller.getBoundingClientRect()
  const into = d.lastY < view.top + EDGE ? d.lastY - (view.top + EDGE) : d.lastY > view.bottom - EDGE ? d.lastY - (view.bottom - EDGE) : 0
  if (into) scroller.scrollTop += Math.sign(into) * Math.min(16, Math.ceil(Math.abs(into) / 4))
  requestAnimationFrame(scrollAtEdges)
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
  // Home / End: the first and last songs (lib/hotkeys.ts would go to the first and last drawn).
  if ((e.key === 'Home' || e.key === 'End') && windowed.value && e.target === e.currentTarget && !keyMoving.value && !e.ctrlKey && !e.altKey) {
    e.preventDefault()
    return void goTo(e.key === 'Home' ? 0 : props.songs.length - 1)
  }
  if (!(props.queue || props.reorderable) || e.ctrlKey || e.altKey || e.metaKey) return
  const d = drag.value
  if (!keyMoving.value || !d) {
    if (e.key === 'Enter' && e.shiftKey && e.target === e.currentTarget && !props.songs[index].later && !drag.value) {
      e.preventDefault()
      const row = e.currentTarget as HTMLElement
      drag.value = { from: index, to: index, startY: 0, lastY: 0, scrolled: 0, dy: 0, centers: [], height: row.offsetHeight }
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
    const behavior = matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth'
    const row = rowOf(String(key))
    if (row) row.scrollIntoView({ behavior, block: 'center' })
    else if (!scrollToIndex(props.songs.indexOf(song), behavior)) return
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

// The keyboard: one row at a time takes Tab (the one last moved to, or the first drawn when that one isn't); the keys
// within: lib/hotkeys.ts (listKey).
const focused = ref<number | null>(null)
const tabRow = computed(() => {
  const f = focused.value ?? 0
  return f >= shownRows.value.first && f <= shownRows.value.last ? f : shownRows.value.first
})

// ---- Only the rows in and near the view are drawn, the rest space of their height (all of a long list's rows made
// resizing the window and scrolling slow). Each row is as tall as the others (SongRow: 68 px, 60 on a phone); a list
// with headings draws them all. The song held while dragging is always drawn (and the rows between it and the view).
const OVERSCAN = 5
const windowed = computed(() => !rows.value.some((r) => r.heading))
const rowHeight = ref(68)
const shown = ref({ first: 0, last: 2 * OVERSCAN })
let scroller: HTMLElement | null = null
let looking = 0
/** The closest scrolling box around the list (a page, a panel). */
function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let box = el.parentElement; box; box = box.parentElement) {
    const y = getComputedStyle(box).overflowY
    if (y === 'auto' || y === 'scroll') return box
  }
  return null
}
/** Which rows are in and near the view now. */
function look() {
  looking = 0
  const el = list.value
  if (!el || !windowed.value) return
  const count = props.songs.length
  if (!scroller) return void (shown.value = { first: 0, last: count - 1 })
  // Away (its page kept for later): as it was.
  if (!el.offsetParent || !scroller.clientHeight) return
  const row = el.querySelector<HTMLElement>('[data-song-row]')
  if (row?.offsetHeight) rowHeight.value = row.offsetHeight
  const h = rowHeight.value
  const top = el.getBoundingClientRect().top - scroller.getBoundingClientRect().top
  const first = Math.max(0, Math.min(count - 1, Math.floor(-top / h) - OVERSCAN))
  const last = Math.max(first, Math.min(count - 1, Math.ceil((scroller.clientHeight - top) / h) + OVERSCAN))
  if (first !== shown.value.first || last !== shown.value.last) shown.value = { first, last }
}
function scrolled() {
  // Held: the song follows the pointer as the list scrolls under it.
  if (drag.value && !keyMoving.value) placeHeld(drag.value)
  looking ||= requestAnimationFrame(look)
}
const sizes = new ResizeObserver(() => (looking ||= requestAnimationFrame(look)))
onMounted(() => {
  scroller = list.value ? scrollParent(list.value) : null
  if (list.value?.closest('.mobile-shell')) rowHeight.value = 60
  scroller?.addEventListener('scroll', scrolled, { passive: true })
  if (scroller) sizes.observe(scroller)
  look()
})
onActivated(look)
onBeforeUnmount(() => {
  scroller?.removeEventListener('scroll', scrolled)
  sizes.disconnect()
  cancelAnimationFrame(looking)
})
watch(() => props.songs.length, () => nextTick(look))
/** The rows drawn: the ones in view, and the one held (and those between). */
const shownRows = computed(() => {
  const count = rows.value.length
  if (!windowed.value) return { first: 0, last: count - 1 }
  let { first, last } = shown.value
  const held = drag.value?.from
  if (held != null) [first, last] = [Math.min(first, held), Math.max(last, held)]
  return { first: Math.min(first, Math.max(0, count - 1)), last: Math.min(last, count - 1) }
})
const drawn = computed(() => rows.value.slice(shownRows.value.first, shownRows.value.last + 1))
const above = computed(() => shownRows.value.first * rowHeight.value)
const below = computed(() => Math.max(0, rows.value.length - 1 - shownRows.value.last) * rowHeight.value)
/** Song `index` scrolled to the middle of the view (drawn once it's there). False: no scrolling box. */
function scrollToIndex(index: number, behavior: ScrollBehavior = 'auto') {
  if (!scroller || !list.value || index < 0) return false
  const top = list.value.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop
  scroller.scrollTo({ top: top + index * rowHeight.value - (scroller.clientHeight - rowHeight.value) / 2, behavior })
  return true
}
/** The keyboard to song `index` (Home, End): scrolled to, drawn, then focused. */
async function goTo(index: number) {
  if (index < 0) return
  focused.value = index
  scrollToIndex(index)
  look()
  await nextTick()
  rowOf(String(props.songs[index].key ?? props.songs[index].id))?.focus({ preventScroll: true })
}
</script>

<template>
  <div ref="list" class="@container" :data-moving="keyMoving || undefined" :class="{ 'song-on-card': onCard, 'song-strong': queue }" role="list">
    <div v-if="above" aria-hidden="true" :style="{ height: `${above}px` }" />
    <template v-for="{ song, index, heading } in drawn" :key="song.key ?? song.id">
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
        :tabindex="index === tabRow ? 0 : -1"
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
    <div v-if="below" aria-hidden="true" :style="{ height: `${below}px` }" />
  </div>
</template>
