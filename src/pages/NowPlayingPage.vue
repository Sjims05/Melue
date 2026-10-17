<script setup lang="ts">
/**
 * Now playing (Melue's design): the big cover; where it plays from, the song, volume, the seek bar, the controls, and
 * Connect device (planned: Phase 4), Like, More actions (the song's menu); Lyrics (planned), beside them when there's
 * room for three columns (as in the concept at 1920 px), under them otherwise. The middle is as tall as the cover. Under it, Up next (the
 * queue after the playing song): white or like the cards (Display settings), opened with its chevron or the player
 * bar's queue button (?queue=open), with drag, ✕, Clear and double-click to play.
 */
import { ChevronDown, Ellipsis, Heart, Library, MonitorSpeaker, Music, Repeat, Search, Shuffle, Volume1, Volume2, VolumeX } from '@lucide/vue'
import { popHeart } from '@/components/player/heart'
import { NextIcon, PauseIcon, PlayIcon, PreviousIcon } from '@/components/player/icons'
import { useResizeObserver } from '@vueuse/core'
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { PlannedCard } from '@/components/page'
import Slider from '@/components/player/Slider.vue'
import { songMenu, SongList } from '@/components/songs'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { appearance } from '@/lib/appearance'
import {
  clearUpNext,
  current,
  currentArtistParts,
  currentCoverLarge,
  fromPage,
  cycleRepeat,
  like,
  moveInUpNext,
  next,
  nowPlaying,
  playFromUpNext,
  position,
  previous,
  removeFromUpNext,
  seek,
  setVolume,
  toggleShuffle,
  togglePlay,
  upNext,
  upNextShown,
} from '@/player/nowPlaying'

const route = useRoute()
const state = computed(() => nowPlaying.state)
const time = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

// Up next: opened with its chevron, or by the player bar's queue button.
const open = ref(route.query.queue === 'open')
watch(() => route.query.queue, (q) => q === 'open' && (open.value = true))

// The volume moves with the slider at once; the player follows.
const volume = ref<number | null>(null)
const shownVolume = computed(() => volume.value ?? state.value?.volume ?? 1)
const VolumeIcon = computed(() => (shownVolume.value === 0 ? VolumeX : shownVolume.value < 0.5 ? Volume1 : Volume2))
function changeVolume(v: number) {
  volume.value = v
  setVolume(v).finally(() => (volume.value = null))
}

// The song: its title takes up to two lines and the artist follows right under it. The volume sits where a two-line
// title's artist would be, so only then does the artist stop ("…") before it.
const title = ref<HTMLElement | null>(null)
const twoLines = ref(false)
useResizeObserver(title, () => {
  const h1 = title.value
  if (h1) twoLines.value = h1.offsetHeight > parseFloat(getComputedStyle(h1).lineHeight) * 1.5
})

// Clear: the songs fold away together (shown open), then they're taken out.
const upNextList = ref<InstanceType<typeof SongList> | null>(null)
function clear() {
  if (open.value && upNextList.value) upNextList.value.foldAwayAll(upNextShown.value.map((u) => u.song), clearUpNext)
  else clearUpNext()
}

const menu = computed(() => (current.value ? songMenu(current.value).filter((i) => i.action !== 'playNext' && i.action !== 'addToQueue') : []))
// A button's colour is one class or the other (two colour classes: the stylesheet's order would decide, not this).
const BIG = 'relative grid size-11 place-items-center rounded-md transition-colors hover:text-foreground [&_svg]:size-5'
const tone = (on: boolean) => (on ? 'text-foreground' : 'text-muted-foreground')
</script>

<template>
  <div class="pt-2">
    <div v-if="current && state" class="grid gap-x-12 gap-y-8 lg:grid-cols-[minmax(0,384px)_minmax(0,1fr)] 3xl:grid-cols-[384px_minmax(0,1fr)_384px]">
      <!-- The cover. -->
      <div class="relative aspect-square w-full overflow-hidden rounded-xl bg-well">
        <Transition name="fade">
          <img v-if="currentCoverLarge" :key="currentCoverLarge" :src="currentCoverLarge" alt="" class="absolute inset-0 size-full object-cover" draggable="false" />
        </Transition>
      </div>

      <!-- The song and its controls. -->
      <div class="flex min-w-0 flex-col justify-between gap-3">
        <!-- As tall as a two-line title with its artist, always (the invisible copy), so nothing under it moves. -->
        <div class="relative grid min-w-0">
          <div aria-hidden="true" class="invisible col-start-1 row-start-1">
            <p class="text-[17px]">&nbsp;</p>
            <p class="mt-2 h-[2lh] text-4xl leading-tight">&nbsp;</p>
            <p class="mt-1 text-lg">&nbsp;</p>
          </div>
          <Transition name="swap" mode="out-in">
            <div :key="current.id" class="col-start-1 row-start-1 min-w-0">
              <!-- Where it plays from, and the artists: each links to its page (underlined under the pointer). -->
              <p class="truncate text-[17px] text-muted-foreground">
                Playing from
                <RouterLink v-if="fromPage" :to="fromPage" class="font-semibold text-foreground hover:underline">{{ nowPlaying.from }}</RouterLink>
                <b v-else class="font-semibold text-foreground">{{ nowPlaying.from }}</b>
              </p>
              <h1 ref="title" class="mt-2 line-clamp-2 text-4xl leading-tight font-bold tracking-tight">{{ current.title }}</h1>
              <!-- Beside the volume (a two-line title) it stops before it: the volume's width and a gap. -->
              <p class="mt-1 truncate text-lg text-muted-foreground" :class="twoLines && 'pr-[232px]'"><template v-for="(part, i) in currentArtistParts" :key="i"><RouterLink v-if="part.artist" :to="`/artist/${part.artist}`" class="hover:text-foreground hover:underline">{{ part.text }}</RouterLink><template v-else>{{ part.text }}</template></template></p>
            </div>
          </Transition>
          <div class="absolute right-0 bottom-0 flex h-7 items-center gap-3">
            <Slider class="w-44" :value="shownVolume" :max="1" :step="0.05" label="Volume" @live="changeVolume" @change="changeVolume" />
            <component :is="VolumeIcon" class="size-5 text-muted-foreground" />
          </div>
        </div>
        <div>
          <Slider :value="position" :max="state.duration || 1" label="Seek" @change="seek" />
          <div class="mt-1 flex justify-between text-sm text-muted-foreground tabular-nums">
            <span>{{ time(position) }}</span><span>{{ time(state.duration) }}</span>
          </div>
        </div>
        <div class="flex items-center justify-between">
          <button type="button" :class="[BIG, tone(state.shuffle)]" :aria-pressed="state.shuffle" aria-label="Shuffle" @click="toggleShuffle">
            <Shuffle />
            <span v-if="state.shuffle" class="absolute bottom-0 size-1 rounded-full bg-current" />
          </button>
          <button type="button" :class="[BIG, tone(true)]" aria-label="Previous" @click="previous"><PreviousIcon /></button>
          <button
            type="button"
            class="grid size-[60px] place-items-center rounded-full bg-foreground text-background transition-transform active:scale-95"
            :aria-label="state.isPlaying ? 'Pause' : 'Play'"
            @click="togglePlay"
          >
            <PauseIcon v-if="state.isPlaying" class="size-6" />
            <PlayIcon v-else class="size-6 translate-x-0.5" />
          </button>
          <button type="button" :class="[BIG, tone(true)]" aria-label="Next" @click="next"><NextIcon /></button>
          <button type="button" :class="[BIG, tone(state.repeatMode !== 'off')]" :aria-label="`Repeat: ${state.repeatMode}`" @click="cycleRepeat">
            <Repeat />
            <span v-if="state.repeatMode === 'one'" class="absolute top-0.5 right-0.5 text-[10px] leading-none font-semibold">1</span>
            <span v-if="state.repeatMode !== 'off'" class="absolute bottom-0 size-1 rounded-full bg-current" />
          </button>
        </div>
        <div class="flex flex-wrap justify-center gap-4">
          <Button variant="outline" disabled title="Planned: play on another device (Phase 4)"><MonitorSpeaker />Connect device</Button>
          <Button variant="outline" @click="(e: MouseEvent) => (popHeart(e.currentTarget, !current!.liked), like(Number(current!.id), !current!.liked))">
            <Heart class="heart" :class="current.liked && 'fill-current'" />
            <!-- Both words in one place (the button keeps its width), the one shown fading in. -->
            <span class="grid">
              <span class="col-start-1 row-start-1 transition-opacity duration-200" :class="current.liked && 'opacity-0'">Like</span>
              <span class="col-start-1 row-start-1 transition-opacity duration-200" :class="!current.liked && 'opacity-0'">Liked</span>
            </span>
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button variant="outline"><Ellipsis />More actions</Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="center" class="min-w-48">
              <template v-for="item in menu" :key="item.action">
                <DropdownMenuSeparator v-if="item.apart" />
                <DropdownMenuItem
                  :variant="item.danger ? 'destructive' : 'default'"
                  @select="item.action === 'like' && like(Number(current.id), !current.liked)"
                >
                  <component :is="item.icon" :class="item.filled && 'fill-current'" />{{ item.label }}
                </DropdownMenuItem>
              </template>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <!-- Lyrics: planned. -->
      <PlannedCard class="h-full lg:col-span-2 3xl:col-span-1" title="Lyrics" text="Planned: the song's lyrics, read from its file, following along as it plays." />
    </div>

    <!-- Nothing playing: the page is there, and says what to do, in the middle of the window. -->
    <div v-else class="grid min-h-[calc(100dvh-88px-8rem)] place-items-center">
      <div class="flex max-w-xl flex-col items-center text-center">
        <span class="grid size-24 place-items-center rounded-2xl bg-well text-muted-foreground"><Music class="size-10" /></span>
        <h1 class="mt-6 text-4xl font-bold tracking-tight">Nothing playing</h1>
        <p class="mt-3 text-lg text-muted-foreground">Play a song, an album or a playlist, and it shows up here.</p>
        <div class="mt-8 flex gap-3">
          <Button as-child><RouterLink to="/library"><Library />Go to Library</RouterLink></Button>
          <Button variant="outline" as-child><RouterLink to="/search"><Search />Search</RouterLink></Button>
        </div>
      </div>
    </div>

    <!-- Up next. -->
    <section
      v-if="current"
      class="mx-auto mt-12 max-w-[1008px] rounded-xl"
      :class="appearance.upNextWhite ? 'light-area' : 'bg-card text-card-foreground'"
    >
      <div class="flex items-center gap-4 p-4 pl-5">
        <button type="button" class="flex min-w-0 flex-1 flex-col text-left" :aria-expanded="open" @click="open = !open">
          <b class="text-[15px] font-semibold">Up next · {{ upNext.length }} song{{ upNext.length === 1 ? '' : 's' }}</b>
          <span class="text-[15px] text-muted-foreground">
            From {{ nowPlaying.from }}<template v-if="state?.repeatMode === 'one'"> · Repeating this song</template>
          </span>
        </button>
        <button
          type="button"
          class="grid size-9 place-items-center rounded-md transition-colors hover:bg-well"
          :aria-label="open ? 'Close Up next' : 'Open Up next'"
          @click="open = !open"
        >
          <ChevronDown class="size-5 transition-transform duration-300" :class="open && 'rotate-180'" />
        </button>
        <Button size="sm" :disabled="!upNext.length" @click="clear">Clear</Button>
      </div>
      <div class="grid transition-[grid-template-rows] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]" :class="open ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'">
        <!-- Clipped as it opens and closes; open, with room around it for a picked-up song (it grows) and its shadow. -->
        <div class="min-h-0" :class="open ? 'overflow-clip [overflow-clip-margin:32px]' : 'overflow-hidden'">
          <SongList
            v-if="upNext.length"
            ref="upNextList"
            class="px-2 pb-3"
            :songs="upNextShown.map((u) => u.song)"
            queue
            plays-next
            on-card
            @play="(song) => playFromUpNext(upNextShown.find((u) => u.song.key === song.key)!.index)"
            @remove="(_, i) => removeFromUpNext(i)"
            @move="moveInUpNext"
            @like="(song, liked) => like(Number(song.id), liked)"
          />
          <p v-else class="px-5 pb-5 text-sm text-muted-foreground">Nothing queued after this song.</p>
        </div>
      </div>
    </section>
  </div>
</template>
