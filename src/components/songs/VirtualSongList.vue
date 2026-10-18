<script setup lang="ts">
/**
 * A long list of songs (the whole library) that only draws the rows on screen, plus a screen's worth above and below
 * so scrolling never shows them appear (@tanstack/vue-virtual). It scrolls with the page: the page's scroller is the
 * nearest `[data-scroller]` around it. Optional headings between the rows (A–Z: `headingOf` gives each song's, and
 * one shows where it changes). The rows are the usual SongRow: the playing song highlighted with its bars, ▶ ♥ ⋯.
 */
import { useVirtualizer } from '@tanstack/vue-virtual'
import { computed, onActivated, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import SongRow from './SongRow.vue'
import type { Song, SongAction, SongMenuPlace } from './songMenu'

const props = defineProps<{
  songs: Song[]
  /** Each song's heading (a letter); one shows before the first song of each. None: no headings. */
  headingOf?: (song: Song) => string
  /** The song playing (or paused on). */
  currentId?: Song['id'] | null
  /** Playing, not paused. */
  playing?: boolean
  place?: SongMenuPlace
  /** A number before each song, in the queue's place (its play count, the date it was added), and what it means. */
  numberOf?: (song: Song) => { text: string; title: string }
}>()
const emit = defineEmits<{
  /** `via`: its play button, or a double-click on the row. */
  play: [song: Song, via: 'button' | 'doubleClick']
  pause: []
  like: [song: Song, liked: boolean]
  action: [song: Song, action: SongAction]
}>()

// A song row's height, and a heading's (with the room above it).
const ROW = 68
const HEADING = 44
// Rows drawn beyond the screen, above and below (about a screen's worth).
const OVERSCAN = 14

type Item = { key: string | number; heading: string } | { key: string | number; song: Song }
const items = computed<Item[]>(() => {
  const out: Item[] = []
  let last: string | null = null
  for (const song of props.songs) {
    // An empty heading shows nothing (a folder's own songs, before its subfolders').
    const heading = props.headingOf?.(song)
    if (heading != null && heading !== last) {
      if (heading) out.push({ key: `h:${heading}:${out.length}`, heading })
      last = heading
    }
    out.push({ key: song.key ?? song.id, song })
  }
  return out
})

const list = ref<HTMLElement>()
const scroller = ref<HTMLElement | null>(null)
// Where the list starts in the page's scrolling content (the page's header is above it). Measured again as the page
// scrolls, and once it has slid in: measured while the page it replaces is still there, it'd be off by that page's
// height (rows would appear and go in view).
const margin = ref(0)
function locate() {
  const el = list.value
  const found = el?.closest<HTMLElement>('[data-scroller]') ?? null
  if (found !== scroller.value) {
    scroller.value?.removeEventListener('scroll', locate)
    found?.addEventListener('scroll', locate, { passive: true })
    scroller.value = found
  }
  if (!el || !found || !el.isConnected) return
  const top = Math.round(el.getBoundingClientRect().top - found.getBoundingClientRect().top + found.scrollTop)
  if (top !== margin.value) margin.value = top
}
const settle = () => {
  locate()
  setTimeout(locate, 400)
}
onMounted(settle)
onActivated(settle)
onBeforeUnmount(() => scroller.value?.removeEventListener('scroll', locate))
watch(() => props.songs, () => requestAnimationFrame(locate))

const virtualizer = useVirtualizer(
  computed(() => ({
    count: items.value.length,
    getScrollElement: () => scroller.value,
    estimateSize: (i: number) => ('heading' in items.value[i] ? HEADING : ROW),
    getItemKey: (i: number) => items.value[i].key,
    overscan: OVERSCAN,
    scrollMargin: margin.value,
  })),
)
</script>

<template>
  <div ref="list" class="@container relative" role="list" :style="{ height: `${virtualizer.getTotalSize()}px` }">
    <div
      v-for="row in virtualizer.getVirtualItems()"
      :key="row.key as string | number"
      class="absolute inset-x-0 top-0"
      :style="{ height: `${row.size}px`, transform: `translateY(${row.start - margin}px)` }"
      :role="'song' in items[row.index] ? 'listitem' : undefined"
    >
      <template v-for="item in [items[row.index]]" :key="item.key">
        <div v-if="'heading' in item" class="flex h-full items-end px-2 pb-1 text-sm font-semibold text-muted-foreground">{{ item.heading }}</div>
        <SongRow
          v-else
          :song="item.song"
          :number="numberOf?.(item.song).text"
          :number-title="numberOf?.(item.song).title"
          wide-number
          :current="item.song.id === currentId"
          :playing="playing"
          :place="place"
          @play="(via) => emit('play', item.song, via)"
          @pause="emit('pause')"
          @like="(liked) => emit('like', item.song, liked)"
          @action="(action) => emit('action', item.song, action)"
        />
      </template>
    </div>
  </div>
</template>
