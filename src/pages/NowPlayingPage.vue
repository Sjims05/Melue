<script setup lang="ts">
/**
 * Now playing (Melue's design): the big cover; where it plays from, the song, volume, the seek bar, the controls, and
 * Connect device (player/remotePlayback.ts: play on another of your devices), Like, More actions (the song's menu); Lyrics (LyricsPanel.vue: following the
 * song, in the cover's colours; full size from its button, a step in the history), beside them when there's
 * room for three columns (as in the concept at 1920 px), under them otherwise. The middle is as tall as the cover. Under it, Up next (the
 * queue after the playing song): white or like the cards (Display settings), opened with its chevron or the player
 * bar's queue button (?queue=open), with drag, ✕, Clear and double-click to play.
 */
import { ChevronDown, Ellipsis, Library, MonitorSpeaker, Music, Repeat, Search, Shuffle, Volume1, Volume2, VolumeX } from '@lucide/vue'
import LikeHeart from '@/components/player/LikeHeart.vue'
import LyricsPanel from '@/components/player/LyricsPanel.vue'
import { NextIcon, PauseIcon, PlayIcon, PreviousIcon } from '@/components/player/icons'
import { useEventListener, useResizeObserver } from '@vueuse/core'
import { computed, nextTick, onActivated, onDeactivated, ref, watch } from 'vue'
import { volumeFlash } from '@/lib/hotkeys'
import DevicesMenu from '@/components/player/DevicesMenu.vue'
import { openDevicesDrawer, playingOn } from '@/player/remotePlayback'
import { connectedTo } from '@/player/playingOn'
import { useRoute, useRouter } from 'vue-router'
import Slider from '@/components/player/Slider.vue'
import { AddToPlaylist, songMenu, SongList, SongMenuArea, type SongAction } from '@/components/songs'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { appearance, primaryArea } from '@/lib/appearance'
import { isDesktop } from '@/platform/platform'
import { followLyrics } from '@/player/lyrics'
import { upNextOpen } from '@/player/upNext'
import { songAction } from '@/player/songs'
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
  snapVolume,
  volumeLabel,
  volumeStep,
  volumeSteps,
  toggleShuffle,
  togglePlay,
  upNext,
  upNextShown,
} from '@/player/nowPlaying'
import { openMenu } from '@/lib/menuDrawer'
import { usePhone } from '@/lib/phone'

const route = useRoute()
const state = computed(() => nowPlaying.state)
const time = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

// Up next: opened with its chevron, or by the player bar's queue button.
// The player bar's queue button (?queue=open) opens it, and the page shows it: coming from another page, already
// there as the page slides in (scrolling as well, everything would rise twice over); here already, scrolled down to
// it smoothly. Aimed again each frame: Up next is still opening, so how far the page can scroll grows as it goes. It
// goes only as far as it takes to show all of Up next (the player stays in view above a short one), or its top just
// under the top bar when it's taller than the window.
const router = useRouter()

// Lyrics: read for each song that plays; full size as a step in the history (back, Escape or ✕ closes it).
followLyrics()
const lyricsFull = computed(() => route.name === 'now-playing' && 'lyrics' in route.query)
const openLyrics = () => router.push({ query: { ...route.query, lyrics: '1' } })
const closeLyrics = () => router.back()
useEventListener('keydown', (e: KeyboardEvent) => lyricsFull.value && e.key === 'Escape' && closeLyrics())

// Open or not: shared (player/upNext.ts), so the Q key can toggle it from anywhere.
const open = upNextOpen
if (route.query.queue === 'open') open.value = true
const upNextPanel = ref<HTMLElement>()
const SCROLL_MS = 600
const ROOM = 16
/** How long Up next takes to open (its grid-rows transition): followed that long when the page comes in. */
const OPENING_MS = 340
async function showQueue(arriving: boolean) {
  if (route.query.queue !== 'open') return
  open.value = true
  router.replace({ query: {} })
  await nextTick()
  const panel = upNextPanel.value
  const scroller = panel?.closest<HTMLElement>('[data-scroller]')
  if (!panel || !scroller) return
  // After the frame has put back where the page was scrolled (its own nextTick), from there.
  requestAnimationFrame((start) => {
    const from = scroller.scrollTop
    const ease = (t: number) => 1 - Math.pow(1 - t, 3)
    const step = (now: number) => {
      const t = arriving ? 1 : Math.min(1, (now - start) / SCROLL_MS)
      const box = panel.getBoundingClientRect()
      const view = scroller.getBoundingClientRect()
      const top = box.top - view.top + scroller.scrollTop - ROOM
      const allShown = box.bottom - view.top + scroller.scrollTop + ROOM - scroller.clientHeight
      const to = Math.max(from, Math.min(top, allShown))
      scroller.scrollTop = from + (to - from) * ease(t)
      if (t < 1 || (arriving && now - start < OPENING_MS)) requestAnimationFrame(step)
    }
    step(start)
  })
}
watch(
  () => route.query.queue,
  () => showQueue(false),
)
onActivated(() => showQueue(true))

// The position, only while this page is shown: put aside (another page in front), it would re-render a few times a
// second for nothing.
const shown = ref(true)
onActivated(() => (shown.value = true))
onDeactivated(() => (shown.value = false))
const at = computed(() => (shown.value ? position.value : 0))

// The volume moves with the slider at once; the player follows.
const volume = ref<number | null>(null)
const shownVolume = computed(() => volume.value ?? state.value?.volume ?? 1)
const VolumeIcon = computed(() => (shownVolume.value === 0 ? VolumeX : shownVolume.value < 0.5 ? Volume1 : Volume2))
// The level beside the slider a moment, while it changes: with the mouse, or the + and − keys (lib/hotkeys.ts).
const volumeShown = ref(false)
let volumeHiding: ReturnType<typeof setTimeout> | undefined
function showVolume() {
  volumeShown.value = true
  clearTimeout(volumeHiding)
  volumeHiding = setTimeout(() => (volumeShown.value = false), 1200)
}
watch(() => volumeFlash.at, showVolume)
function changeVolume(v: number) {
  showVolume()
  volume.value = snapVolume(v)
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
// Up next's last songs folding away (Clear, or the last one taken out): "Nothing queued" opens as they close, so the
// space stops where it will be. (With repeat all the next pass shows after them: not empty.)
const emptying = ref(false)
// Shown by what's in the list as shown: on the last song with repeat all, Up next itself is empty, but the next pass
// shows (greyed).
const emptyShown = computed(() => emptying.value || !upNextShown.value.length)
function folding(count: number) {
  if (count >= upNextShown.value.length && nowPlaying.state?.repeatMode !== 'all') emptying.value = true
}
watch(
  () => upNextShown.value.length,
  (n) => n === 0 && (emptying.value = false),
)
function clear() {
  if (open.value && upNextList.value) upNextList.value.foldAwayAll(upNextShown.value.map((u) => u.song), clearUpNext)
  else clearUpNext()
}

const menu = computed(() => (current.value ? songMenu(current.value).filter((i) => i.action !== 'playNext' && i.action !== 'addToQueue') : []))
// A phone: More actions opens the song's menu as a drawer.
const phone = usePhone()
// A phone's full-size lyrics have their own controls at the bottom: notes and the "Playing on" pill go above them, just
// over the lyrics' fade (shells/Toasts.vue).
watch(() => phone && lyricsFull.value && !!current.value, (on) => document.documentElement.toggleAttribute('data-lyrics-full', on), { immediate: true })
function openSongMenu() {
  const song = current.value
  if (song) openMenu({ title: song.title, subtitle: song.artist, cover: { image: song.cover }, items: menu.value, pick: (a) => songAction(song, a as SongAction), song })
}
// A button's colour is one class or the other (two colour classes: the stylesheet's order would decide, not this).
const BIG = 'relative grid size-11 place-items-center rounded-md transition-colors hover:text-foreground [&_svg]:size-5'
const tone = (on: boolean) => (on ? 'text-foreground' : 'text-muted-foreground')
</script>

<template>
  <div class="pt-2">
    <div v-if="current && state" class="grid gap-x-12 gap-y-8 lg:grid-cols-[minmax(0,384px)_minmax(0,1fr)] 3xl:grid-cols-[384px_minmax(0,1fr)_384px]">
      <!-- The cover; its right-click (a long press on a phone) is the song's menu, like a list page's header. -->
      <SongMenuArea :song="current" :items="menu" @action="(action) => songAction(current!, action)">
        <div class="relative grid aspect-square w-full place-items-center overflow-hidden rounded-xl bg-well text-muted-foreground">
          <!-- No cover (a song only on the other device, or one without): the note, as the mini player shows it. -->
          <Music v-if="!currentCoverLarge" class="size-1/4" />
          <Transition name="fade">
            <img v-if="currentCoverLarge" :key="currentCoverLarge" :src="currentCoverLarge" alt="" class="absolute inset-0 size-full object-cover" draggable="false" />
          </Transition>
        </div>
      </SongMenuArea>

      <!-- The song and its controls. -->
      <div class="flex min-w-0 flex-col justify-between gap-3">
        <!-- As tall as a two-line title with its artist, always (the invisible copy), so nothing under it moves. Its
             right-click (a long press) is the song's menu too. -->
        <SongMenuArea :song="current" :items="menu" @action="(action) => songAction(current!, action)">
        <div class="relative grid min-w-0">
          <div aria-hidden="true" class="invisible col-start-1 row-start-1">
            <p class="text-[17px]">&nbsp;</p>
            <p class="mt-2 h-[2lh] text-4xl leading-tight phone:mt-1 phone:text-[28px]">&nbsp;</p>
            <p class="mt-1 text-lg phone:text-base">&nbsp;</p>
          </div>
          <Transition name="swap" mode="out-in">
            <div :key="current.id" class="col-start-1 row-start-1 min-w-0">
              <!-- Where it plays from, and the artists: each links to its page (underlined under the pointer). -->
              <p v-if="playingOn.device" class="flex items-center gap-1.5 truncate text-[17px] text-muted-foreground">
                <MonitorSpeaker class="size-4 flex-none text-primary" />Playing on <b class="font-semibold text-foreground">{{ playingOn.device.name }}</b>
              </p>
              <p v-else class="truncate text-[17px] text-muted-foreground">
                Playing from
                <RouterLink v-if="fromPage" :to="fromPage" class="font-semibold text-foreground hover:underline">{{ nowPlaying.from }}</RouterLink>
                <b v-else class="font-semibold text-foreground">{{ nowPlaying.from }}</b>
              </p>
              <h1 ref="title" class="mt-2 line-clamp-2 text-4xl leading-tight font-bold tracking-tight phone:mt-1 phone:text-[28px]">{{ current.title }}</h1>
              <!-- Beside the volume (a two-line title) it stops before it: the volume's width and a gap. -->
              <p class="mt-1 truncate text-lg text-muted-foreground phone:pr-0! phone:text-base" :class="twoLines && 'pr-[232px]'"><template v-for="(part, i) in currentArtistParts" :key="i"><RouterLink v-if="part.artist" :to="`/artist/${part.artist}`" class="hover:text-foreground hover:underline">{{ part.text }}</RouterLink><template v-else>{{ part.text }}</template></template></p>
            </div>
          </Transition>
          <!-- Not on a phone: its own volume buttons. The arrows pass over the player's controls (their keys do them). -->
          <div class="absolute right-0 bottom-0 flex h-7 items-center gap-3 phone:hidden" data-arrows-skip>
            <Transition name="fade">
              <span v-if="volumeShown" class="min-w-8 text-right text-sm text-muted-foreground tabular-nums">{{ volumeLabel(shownVolume) }}</span>
            </Transition>
            <Slider class="w-44" :value="shownVolume" :max="1" :step="volumeStep()" :ticks="volumeSteps" label="Volume" @live="changeVolume" @change="changeVolume" />
            <component :is="VolumeIcon" class="size-5 text-muted-foreground" />
          </div>
        </div>
        </SongMenuArea>
        <div data-arrows-skip>
          <Slider :value="at" :max="state.duration || 1" label="Seek" @change="seek" />
          <div class="mt-1 flex justify-between text-sm text-muted-foreground tabular-nums">
            <span>{{ time(at) }}</span><span>{{ time(state.duration) }}</span>
          </div>
        </div>
        <div class="flex items-center justify-between" data-arrows-skip>
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
          <Button v-if="phone" variant="outline" @click="openDevicesDrawer"><MonitorSpeaker />{{ connectedTo() ?? 'Connect device' }}</Button>
          <DevicesMenu v-else side="bottom" align="center">
            <Button variant="outline"><MonitorSpeaker />{{ connectedTo() ?? 'Connect device' }}</Button>
          </DevicesMenu>
          <Button variant="outline" @click="like(Number(current!.id), !current!.liked)">
            <LikeHeart :liked="!!current.liked" />
            <!-- Both words in one place (the button keeps its width), the one shown fading in. -->
            <span class="grid">
              <span class="col-start-1 row-start-1 transition-opacity duration-200" :class="current.liked && 'opacity-0'">Like</span>
              <span class="col-start-1 row-start-1 transition-opacity duration-200" :class="!current.liked && 'opacity-0'">Liked</span>
            </span>
          </Button>
          <Button v-if="phone" variant="outline" @click="openSongMenu"><Ellipsis />More actions</Button>
          <DropdownMenu v-else>
            <DropdownMenuTrigger as-child>
              <Button variant="outline"><Ellipsis />More actions</Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="center" class="min-w-48">
              <template v-for="item in menu" :key="item.action">
                <DropdownMenuSeparator v-if="item.apart" />
                <AddToPlaylist v-if="item.action === 'addToPlaylist'" :song="current" in="dropdown" />
                <DropdownMenuItem
                  v-else
                  :variant="item.danger ? 'destructive' : 'default'"
                  @select="songAction(current, item.action)"
                >
                  <component :is="item.icon" :class="item.filled && 'fill-current'" />{{ item.label }}
                </DropdownMenuItem>
              </template>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <!-- Lyrics: beside the controls (three columns), as tall as the cover and no taller (it doesn't make the row grow:
           it fills its place); under them, a few lines. -->
      <div class="relative h-44 lg:col-span-2 3xl:col-span-1 3xl:h-auto">
        <LyricsPanel class="absolute inset-0" @expand="openLyrics" />
      </div>
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

    <!-- Lyrics full size: on a computer over the page (the top bar and the player bar stay, for the window and the
         controls); on a phone over everything, with its own controls. -->
    <Teleport to="#page-overlay" :disabled="phone">
      <Transition name="fade">
        <div v-if="lyricsFull && current" class="z-30 bg-background pt-[env(safe-area-inset-top)]" :class="phone ? 'fixed inset-0' : 'absolute inset-0'">
          <LyricsPanel big :controls="phone" class="h-full" @close="closeLyrics" />
        </div>
      </Transition>
    </Teleport>

    <!-- Up next. -->
    <section
      v-if="current"
      id="up-next"
      ref="upNextPanel"
      class="mx-auto mt-12 max-w-[1008px] scroll-mt-6 rounded-xl"
      :class="appearance.upNextPrimary ? primaryArea : 'bg-card text-card-foreground'"
    >
      <div class="flex items-center gap-4 p-4 pl-5">
        <button type="button" class="flex min-w-0 flex-1 flex-col text-left" :aria-expanded="open" @click="open = !open">
          <!-- On the last song with repeat all: nothing before it starts over (the next pass shows, greyed). -->
          <b class="text-[15px] font-semibold">{{
            !upNext.length && upNextShown.length ? 'Up next · Starts over next' : `Up next · ${upNext.length} song${upNext.length === 1 ? '' : 's'}`
          }}</b>
          <span class="text-[15px] text-muted-foreground">
            <!-- A long name goes to the next line whole (cut short if it's longer than a line), not word by word. -->
            From <span class="inline-block max-w-full truncate align-bottom">{{ nowPlaying.from }}</span><template v-if="state?.repeatMode === 'one'"> · Repeating this song</template>
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
          <!-- There even when empty, so the first song added unfolds into it like the others (SongList). -->
          <SongList
            ref="upNextList"
            class="px-2 transition-[padding] duration-[260ms] ease-[cubic-bezier(.2,.8,.2,1)]"
            :class="emptying || !upNextShown.length ? 'pb-0' : 'pb-3'"
            :songs="upNextShown.map((u) => u.song)"
            queue
            plays-next
            on-card
            @play="(song) => playFromUpNext(upNextShown.find((u) => u.song.key === song.key)!.index)"
            @remove="(_, i) => removeFromUpNext(i)"
            @folding="folding"
            @move="moveInUpNext"
            @like="(song, liked) => like(Number(song.id), liked)"
            @action="songAction"
            :place="{ showInFolder: isDesktop }"
          />
          <!-- Opens as the last songs fold away (the same time and curve as their fold: SongList's FOLD). -->
          <div
            class="grid transition-[grid-template-rows,opacity] duration-[260ms] ease-[cubic-bezier(.2,.8,.2,1)]"
            :class="emptyShown ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'"
          >
            <div class="min-h-0 overflow-hidden"><p class="px-5 pb-5 text-sm text-muted-foreground">Nothing queued after this song.</p></div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
