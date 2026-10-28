<script setup lang="ts">
/**
 * The playing song's lyrics (player/lyrics.ts), as in the concept: a card on Now playing, or full size (`big`). Timed
 * lyrics follow the song: the line being sung, and its word being sung (enhanced LRC), light up in two colours from the
 * cover (lib/coverColors.ts: picked to read well on the card), and the card keeps that line in view (it can't be
 * scrolled by hand: a swipe over it scrolls the page; full size can, and is left alone a moment after). A tap on a timed line plays from it. The timing tools move the song's lyrics
 * earlier or later (a quarter second a press), or, with "Sync by tapping", set a line where it's tapped (the lines
 * after it follow: player/lyrics.ts timedLines); kept for the song. Lyrics without times get a rough timing (marked
 * as estimated) until they're tapped.
 */
import { LoaderCircle, Maximize2, Minus, Plus, Pointer, Timer, X } from '@lucide/vue'
import { computed, nextTick, onActivated, ref, watch } from 'vue'
import { lyricColors, parseOklch, type LyricColors } from '@/lib/coverColors'
import { isDark } from '@/lib/theme'
import { clearTaps, lyrics, nudgeTap, removeTap, setLyricsOffset, tapLine, timedLines, tryOtherVersion, useLyricsClock } from '@/player/lyrics'
import { toast } from '@/lib/toast'
import { Reveal } from '@/components/settings'
import { NextIcon, PauseIcon, PlayIcon, PreviousIcon } from '@/components/player/icons'
import { current, next, nowPlaying, previous, seek, togglePlay } from '@/player/nowPlaying'
import { usePhone } from '@/lib/phone'

/** `controls`: previous, play / pause and next under it (full size on a phone, where nothing else shows them). */
const props = defineProps<{ big?: boolean; controls?: boolean }>()
const emit = defineEmits<{ expand: []; close: [] }>()
const phone = usePhone()

const ms = useLyricsClock()
const offset = computed(() => lyrics.value?.offsetMs ?? 0)

// Adjusting the timing: a quarter second a press.
const STEP_MS = 250
const TOOL =
  'grid size-8 place-items-center rounded-md text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50'
const adjusting = ref(false)
const shift = (by: number) => setLyricsOffset(offset.value + by)
// Lyrics from LRCLIB that don't fit (timed for another version of the song): its next entry.
const trying = ref(false)
async function otherVersion() {
  trying.value = true
  const found = await tryOtherVersion()
  trying.value = false
  toast(found ? 'Another version of the lyrics' : 'LRCLIB has no other version of these lyrics')
}
const offsetWords = computed(() => {
  const s = (Math.abs(offset.value) / 1000).toFixed(2)
  return offset.value === 0 ? 'In time' : offset.value > 0 ? `${s} s later` : `${s} s earlier`
})
// The lines with the timing set by hand (moved, tapped).
const lines = computed(() => (lyrics.value ? timedLines(lyrics.value) : []))
// "Sync by tapping": a tap on a line sets it to now (instead of playing from it).
const tapping = ref(false)
const taps = computed(() => lyrics.value?.anchors.length ?? 0)
// A tapped line's tap (ms into the song), and fine-tuning it a tenth of a second at a time.
const tapAt = (line: number) => lyrics.value?.anchors.find((a) => a.line === line)?.atMs ?? null
const NUDGE_MS = 100
const NUDGE = 'grid size-5 place-items-center rounded hover:bg-background hover:text-foreground'
/** "1:23.4" */
const clockOf = (ms: number) => `${Math.floor(ms / 60000)}:${((ms % 60000) / 1000).toFixed(1).padStart(4, '0')}`
watch(adjusting, (on) => !on && (tapping.value = false))
watch(() => current.value?.id, () => (tapping.value = false))
const synced = computed(() => !!lyrics.value?.synced)

// The line being sung: the last one started (a moment early, so it lights up as it's sung, not after).
const LEAD_MS = 150
const currentLine = computed(() => {
  if (!synced.value) return -1
  const at = ms.value + LEAD_MS
  let found = -1
  for (let i = 0; i < lines.value.length; i++) {
    const t = lines.value[i].atMs
    if (t != null && t <= at) found = i
    else if (t != null) break
  }
  return found
})
const currentWord = computed(() => {
  const words = lines.value[currentLine.value]?.words ?? []
  let found = -1
  for (let i = 0; i < words.length && words[i].atMs <= ms.value + LEAD_MS; i++) found = i
  return found
})

// Colours from the cover, readable on this card (its colour, from the theme).
const panel = ref<HTMLElement>()
const colors = ref<LyricColors | null>(null)
async function pickColors() {
  const cover = current.value?.cover
  if (!cover) return void (colors.value = null)
  // Only while it's on the page (Now playing set aside: its colour can't be read; done again when it's back).
  if (!panel.value?.isConnected) return
  const bg = parseOklch(getComputedStyle(panel.value).getPropertyValue(props.big ? '--background' : '--card'))
  if (bg) colors.value = await lyricColors(cover, bg)
}
watch([() => current.value?.cover, isDark, panel], pickColors, { immediate: true })
onActivated(pickColors)
const colorVars = computed(() => (colors.value ? { '--lyric-line': colors.value.line, '--lyric-word': colors.value.word } : {}))

// Keeping the line in view: a little above the middle. Scrolled by hand: left alone for a few seconds.
const scroller = ref<HTMLElement>()
const lineEls = ref<HTMLElement[]>([])
let handsOffUntil = 0
const byHand = () => (handsOffUntil = performance.now() + 4000)
function show(index: number, smooth = true) {
  const el = lineEls.value[index]
  const box = scroller.value
  if (!el || !box || performance.now() < handsOffUntil) return
  box.scrollTo({ top: el.offsetTop - box.clientHeight * 0.3, behavior: smooth ? 'smooth' : 'instant' })
}
watch(currentLine, (i) => i >= 0 && show(i))
// A new song (or the full view opening): its place at once.
watch(lyrics, async () => {
  await nextTick()
  handsOffUntil = 0
  if (currentLine.value >= 0) show(currentLine.value, false)
  else scroller.value?.scrollTo({ top: 0 })
})

function lineTapped(index: number) {
  if (tapping.value) return void tapLine(index, ms.value)
  const at = lines.value[index]?.atMs
  if (at == null) return
  handsOffUntil = 0
  seek(Math.max(0, at) / 1000)
}
</script>

<template>
  <section ref="panel" class="lyrics flex min-h-0 flex-col overflow-hidden rounded-xl" :class="big ? 'h-full bg-background' : 'bg-card'" :style="colorVars">
    <header class="flex flex-none items-center justify-between gap-3" :class="big ? 'mx-auto w-full max-w-3xl px-8 pt-8 pb-4 phone:px-5 phone:pt-5' : 'px-4 pt-4 pb-2'">
      <h2 class="font-semibold" :class="big ? 'text-2xl' : 'text-lg'">Lyrics</h2>
      <!-- Timing: its tools open under the header. -->
      <button
        v-if="synced"
        type="button"
        :class="[TOOL, 'ml-auto', adjusting && 'text-foreground']"
        :aria-pressed="adjusting"
        aria-label="Adjust the timing"
        @click="adjusting = !adjusting"
      >
        <Timer class="size-4" />
      </button>
      <button
        v-if="!big"
        type="button"
        class="grid size-8 place-items-center rounded-md text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
        aria-label="Show the lyrics full size"
        @click="emit('expand')"
      >
        <Maximize2 class="size-4" />
      </button>
      <button
        v-else
        type="button"
        class="grid size-10 place-items-center rounded-md text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
        aria-label="Close the lyrics"
        @click="emit('close')"
      >
        <X class="size-5" />
      </button>
    </header>
    <!-- Earlier / later (a quarter second a press; the words: back to as they are), and another version of lyrics found
         online. -->
    <Reveal :show="synced && adjusting" :line="false">
      <div class="flex flex-wrap items-center gap-x-1 gap-y-1 pb-2 text-[13px] text-muted-foreground" :class="big ? 'mx-auto max-w-3xl px-7 phone:px-4' : 'px-3'">
        <button type="button" :class="TOOL" aria-label="Lyrics earlier" @click="shift(-STEP_MS)"><Minus class="size-4" /></button>
        <button type="button" class="min-w-24 rounded-md px-1 tabular-nums hover:text-foreground" title="Back to as they are" @click="setLyricsOffset(0)">{{ offsetWords }}</button>
        <button type="button" :class="TOOL" aria-label="Lyrics later" @click="shift(STEP_MS)"><Plus class="size-4" /></button>
        <button
          v-if="lyrics?.source === 'online'"
          type="button"
          class="ml-auto flex items-center gap-1.5 rounded-md px-1.5 whitespace-nowrap hover:text-foreground disabled:opacity-60"
          :disabled="trying"
          title="The lyrics are timed for another version of the song: LRCLIB's next one"
          @click="otherVersion"
        >
          <LoaderCircle v-if="trying" class="size-3.5 animate-spin" />
          Try another version
        </button>
        <!-- Sync by tapping: on, a tap on a line sets it to now. -->
        <div class="flex w-full items-center gap-2 pt-1">
          <button
            type="button"
            class="rounded-md px-2 py-1 transition-colors"
            :class="tapping ? 'bg-foreground text-background' : 'bg-well hover:text-foreground'"
            :aria-pressed="tapping"
            title="Then tap each line the moment it starts being sung: it and the lines after it follow"
            @click="tapping = !tapping"
          >
            Sync by tapping
          </button>
          <span v-if="tapping" class="min-w-0 truncate">Tap each line as it starts</span>
          <span v-else-if="taps && !big" class="min-w-0 truncate">Fine-tune taps full size</span>
          <button v-if="taps" type="button" class="ml-auto rounded-md px-1.5 whitespace-nowrap hover:text-foreground" @click="clearTaps">
            Clear {{ taps === 1 ? 'tap' : `${taps} taps` }}
          </button>
        </div>
      </div>
    </Reveal>
    <p v-if="lyrics?.estimated && !taps && !adjusting" class="-mt-1 pb-2 text-xs text-muted-foreground" :class="big ? 'mx-auto w-full max-w-3xl px-8 phone:px-5' : 'px-4'">
      Estimated timing: tap lines to set it (the timer)
    </p>
    <!-- The card follows the song by itself and can't be scrolled (a swipe over it scrolls the page: Now playing is
         scrolled more often than the lyrics); full size, they scroll. -->
    <div
      ref="scroller"
      class="lyrics-lines relative min-h-0 flex-1"
      :class="big ? 'overflow-y-auto px-8 pb-[40vh] phone:px-5' : 'overflow-hidden px-4 pb-24'"
      @wheel.passive="byHand"
      @touchmove.passive="byHand"
    >
      <Transition name="part" mode="out-in">
        <p v-if="lyrics === undefined" key="reading" class="text-sm text-muted-foreground">&nbsp;</p>
        <div v-else-if="!lyrics" key="none" class="flex flex-col gap-1 pt-1 text-sm text-muted-foreground">
          <span class="font-medium text-foreground">No lyrics for this song</span>
          <span>Put a .lrc file next to it (same name), or add them to its tags. With Find lyrics online, Melue looks on Wi-Fi.</span>
        </div>
        <div v-else-if="lyrics.instrumental" key="instrumental" class="flex flex-col gap-1 pt-1 text-sm text-muted-foreground">
          <span class="font-medium text-foreground">Instrumental</span>
          <span>This song has no words.</span>
        </div>
        <div v-else :key="String(current?.id)" class="flex flex-col" :class="big ? 'mx-auto max-w-3xl gap-6 phone:gap-4' : 'gap-4'">
          <p
            v-for="(line, i) in lines"
            :key="i"
            :ref="(el) => el && (lineEls[i] = el as HTMLElement)"
            class="lyric-line relative"
            :class="[
              big ? 'text-4xl leading-tight font-bold tracking-tight phone:text-[26px]' : 'text-base leading-snug',
              i === currentLine && 'lyric-current',
              synced && 'cursor-pointer',
            ]"
            @click="lineTapped(i)"
          >
            <template v-if="i === currentLine && line.words.length">
              <span v-for="(word, j) in line.words" :key="j" :class="j === currentWord && 'lyric-word'">{{ word.text }}</span>
            </template>
            <template v-else>{{ line.text || '♪' }}</template>
            <!-- A tapped line (the timing tools open). Full size: its tap beside the text, in the room to its left (on a
                 phone, under it), a tenth of a second earlier or later, or gone; the text stays where it is. The card:
                 a dot in its margin. -->
            <template v-if="adjusting && tapAt(i) != null">
              <span
                v-if="big"
                class="flex w-fit items-center gap-0.5 rounded-md bg-well py-0.5 pr-0.5 pl-1.5 text-xs leading-none font-normal tracking-normal whitespace-nowrap text-muted-foreground"
                :class="phone ? 'mt-1.5' : 'absolute top-[0.45em] right-full mr-4'"
                @click.stop
              >
                <Pointer class="size-3" aria-hidden="true" />
                <button type="button" :class="NUDGE" aria-label="Tap a tenth of a second earlier" @click="nudgeTap(i, -NUDGE_MS)"><Minus class="size-3" /></button>
                <span class="tabular-nums">{{ clockOf(tapAt(i)!) }}</span>
                <button type="button" :class="NUDGE" aria-label="Tap a tenth of a second later" @click="nudgeTap(i, NUDGE_MS)"><Plus class="size-3" /></button>
                <button type="button" :class="NUDGE" aria-label="Remove this tap" @click="removeTap(i)"><X class="size-3" /></button>
              </span>
              <span v-else class="absolute top-[0.55em] -left-2.5 size-1.5 rounded-full bg-current opacity-60" title="Tapped" />
            </template>
          </p>
          <p v-if="lyrics.source === 'online'" class="text-xs text-muted-foreground">Lyrics from LRCLIB</p>
        </div>
      </Transition>
    </div>
    <div v-if="controls" class="flex flex-none items-center justify-center gap-10 pt-2 pb-[calc(env(safe-area-inset-bottom)+1.25rem)]">
      <button type="button" class="grid size-12 place-items-center" aria-label="Previous" @click="previous"><PreviousIcon class="size-6" /></button>
      <button
        type="button"
        class="grid size-[60px] place-items-center rounded-full bg-foreground text-background transition-transform active:scale-95"
        :aria-label="nowPlaying.state?.isPlaying ? 'Pause' : 'Play'"
        @click="togglePlay"
      >
        <PauseIcon v-if="nowPlaying.state?.isPlaying" class="size-6" />
        <PlayIcon v-else class="size-6 translate-x-0.5" />
      </button>
      <button type="button" class="grid size-12 place-items-center" aria-label="Next" @click="next"><NextIcon class="size-6" /></button>
    </div>
  </section>
</template>

<style scoped>
/* The lines fade out at the bottom, as in the concept. */
.lyrics-lines {
  mask-image: linear-gradient(to bottom, black calc(100% - 2.5rem), transparent);
  scrollbar-width: none;
}
.lyric-line {
  color: var(--muted-foreground);
  transition: color 250ms ease-out;
}
/* The line being sung, and its word: the cover's colours (the theme's when the cover has none). */
.lyric-current {
  color: var(--lyric-line, color-mix(in oklab, var(--foreground) 80%, transparent));
}
.lyric-word {
  color: var(--lyric-word, var(--foreground));
  transition: color 120ms ease-out;
}
</style>
