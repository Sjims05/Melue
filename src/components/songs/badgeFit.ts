/**
 * Whether the device badges ("This device", "Not here yet") and the notes with an icon ("Only in Melue") of a list's
 * rows show their words or their icons: the words only when every row in view has room for them (its title in full,
 * them with their words and anything else on its line), so they're all one or the other. Short of room, the notes
 * become icons first, then the badges; a title is cut short only when even the icons don't fit.
 *
 * Each row works out what fits on its title line as the line changes width (one observer for all of them; only the
 * rows in view exist: SongList) and tells its list (provideBadgeLevels); the list takes the least of them.
 */
import { computed, inject, nextTick, onBeforeUnmount, onMounted, provide, reactive, ref, watch, type ComputedRef, type InjectionKey, type Ref } from 'vue'

/** What has its words: 2 the badges and the notes, 1 the badges only, 0 neither. */
type Level = 0 | 1 | 2

/** The line's gap (SongRow: gap-2). */
const GAP = 8
/** The badge besides its words: the icon (10), the gap after it (4), its padding (6 and 6). */
const BADGE = 26
/** An icon alone (14), a little further from what's before it (4: songs.css). */
const ICON = 18

type Levels = { report: (row: symbol, level: Level | null) => void; level: ComputedRef<Level> }
const LEVELS: InjectionKey<Levels> = Symbol('badge levels')

/** A list (SongList, VirtualSongList): its rows' words or icons, decided together. */
export function provideBadgeLevels() {
  const rows = reactive(new Map<symbol, Level>())
  const level = computed(() => Math.min(2, ...rows.values()) as Level)
  provide(LEVELS, { report: (row, l) => (l === null ? rows.delete(row) : rows.set(row, l)), level })
}

const checks = new Map<Element, () => void>()
const sizes = new ResizeObserver((entries) => entries.forEach((entry) => checks.get(entry.target)?.()))

/** `line`: the row's title line (`.song-title`, `.song-device`, `.song-note`); `changes`: what changes its text. */
export function useBadgeFits(line: Ref<HTMLElement | undefined>, changes: () => unknown) {
  const own = ref<Level>(0)
  const list = inject(LEVELS, null)
  const row = Symbol('row')
  function check() {
    const el = line.value
    if (!el) return
    const title = el.querySelector<HTMLElement>('.song-title')
    const badge = el.querySelector<HTMLElement>('.song-device')
    const note = el.querySelector<HTMLElement>('.song-note')
    // Nothing of the kind on this row: it has no say, and its title shrinks as usual.
    if (!badge && !note) {
      el.classList.remove('song-line-fit')
      return void list?.report(row, null)
    }
    // The rest: the title in full, and anything else on the line.
    let others = 0
    for (const part of el.children) if (part !== title && part !== badge && part !== note) others += (part as HTMLElement).offsetWidth + GAP
    const rest = (title?.scrollWidth ?? 0) + others
    // The title doesn't shrink while a badge or note changes (they give way, songs.css), up to the room their icons
    // leave it: cut short only when even those don't fit.
    el.style.setProperty('--song-after', `${others + (badge ? GAP + ICON : 0) + (note ? GAP + ICON : 0)}px`)
    el.classList.add('song-line-fit')
    const badgeWords = BADGE + (badge?.querySelector<HTMLElement>('.song-device-words')?.scrollWidth ?? 0)
    const noteWords = note?.querySelector<HTMLElement>('.song-note-words')?.scrollWidth ?? 0
    const width = (badgeWide: boolean, noteWide: boolean) =>
      rest + (badge ? GAP + (badgeWide ? badgeWords : ICON) : 0) + (note ? GAP + (noteWide ? noteWords : ICON) : 0)
    const room = el.clientWidth + 1
    own.value = width(true, true) <= room ? 2 : width(true, false) <= room ? 1 : 0
    list?.report(row, own.value)
  }
  let watched: Element | undefined
  onMounted(() => {
    watched = line.value
    if (watched) {
      checks.set(watched, check)
      sizes.observe(watched)
    }
    check()
  })
  onBeforeUnmount(() => {
    list?.report(row, null)
    if (!watched) return
    sizes.unobserve(watched)
    checks.delete(watched)
  })
  watch(changes, () => nextTick(check))
  const level = computed(() => (list ? list.level.value : own.value))
  return { badgeWide: computed(() => level.value >= 1), noteWide: computed(() => level.value >= 2) }
}
