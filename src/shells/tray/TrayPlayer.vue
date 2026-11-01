<script setup lang="ts">
/**
 * The tray's player (desktop: src-tauri tray.rs, its own small window above the tray icon): the song and its cover,
 * like; the seek bar; shuffle, previous, play, next, repeat; the volume; then Open Melue and Quit. Nothing playing: the
 * note, and the buttons that need a song off. The same parts as the player bar (PlayerBar.vue). The window is as tall as
 * this (tray_fit). `demo`: shown with that song and state instead of the player's (New in Melue's stage), in place.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { ListMusic, Music, Repeat, Shuffle, Volume1, Volume2, VolumeX } from '@lucide/vue'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import LikeHeart from '@/components/player/LikeHeart.vue'
import Slider from '@/components/player/Slider.vue'
import { NextIcon, PauseIcon, PlayIcon, PreviousIcon } from '@/components/player/icons'
import { current as playing, cycleRepeat, like, next, nowPlaying, position as playingAt, previous, seek, setVolume, toggleShuffle, togglePlay, volumeSteps } from '@/player/nowPlaying'

export type TrayDemo = {
  song: { title: string; artist: string; cover?: string; liked?: boolean }
  /** Seconds into it, and its length. */
  position: number
  duration: number
  isPlaying: boolean
}
const props = defineProps<{ demo?: TrayDemo }>()

const current = computed(() => (props.demo ? { id: 0, ...props.demo.song } : playing.value))
const position = computed(() => (props.demo ? props.demo.position : playingAt.value))
const state = computed(() =>
  props.demo
    ? { isPlaying: props.demo.isPlaying, duration: props.demo.duration, volume: 0.7, shuffle: false, repeatMode: 'off' as const }
    : nowPlaying.state,
)
const empty = computed(() => !current.value || !state.value)
const volume = computed(() => state.value?.volume ?? 1)
const VolumeIcon = computed(() => (volume.value === 0 ? VolumeX : volume.value < 0.5 ? Volume1 : Volume2))
const time = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

// The player bar's small buttons, one colour each (on: the text's; off: muted).
const ICON = 'relative grid size-9 place-items-center rounded-md transition-colors hover:bg-well hover:text-foreground [&_svg]:size-4'
const tone = (on: boolean) => (on ? 'text-foreground' : 'text-muted-foreground')
const OFF = 'pointer-events-none opacity-40'

// The window takes the page's height (its own, in logical pixels).
const page = ref<HTMLElement>()
const sizes = new ResizeObserver(([entry]) => invoke('tray_fit', { height: Math.ceil(entry.borderBoxSize[0].blockSize) }).catch(() => {}))
onMounted(() => !props.demo && page.value && sizes.observe(page.value))
onBeforeUnmount(() => sizes.disconnect())

// Shown, it fades in and drops into place, as a menu does (tray.rs tells it); hidden, it waits faded out. The window's
// own background is the card's, so only what's on it fades.
if (!props.demo) document.documentElement.style.background = 'var(--card)'
const parts = ref<HTMLElement>()
const stops = props.demo
  ? []
  : [
      listen('tray-shown', () =>
        parts.value?.animate([{ opacity: 0, translate: '0 6px' }, { opacity: 1, translate: '0 0' }], { duration: 180, easing: 'cubic-bezier(.2,.8,.2,1)', fill: 'both' }),
      ),
      listen('tray-hidden', () => parts.value?.animate([{ opacity: 0 }], { duration: 0, fill: 'forwards' })),
    ]
onBeforeUnmount(() => stops.forEach((stop) => stop.then((unlisten) => unlisten())))
</script>

<template>
  <div ref="page" class="bg-card p-4 text-foreground select-none">
    <div ref="parts" class="flex flex-col gap-3">
      <!-- The song. -->
      <div class="flex min-w-0 items-center gap-3">
        <span class="relative grid size-13 flex-none place-items-center overflow-hidden rounded-md bg-well text-muted-foreground">
          <Music v-if="!current?.cover" class="size-5" />
          <!-- Another song: its cover fades in over the last (as the player bar's). -->
          <Transition name="fade">
            <img v-if="current?.cover" :key="current.cover" :src="current.cover" alt="" class="absolute inset-0 size-full object-cover" draggable="false" />
          </Transition>
        </span>
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="truncate text-[15px] font-semibold">{{ current?.title ?? 'Nothing playing' }}</span>
          <span class="truncate text-[13px] text-muted-foreground">{{ current?.artist ?? 'Play a song in Melue' }}</span>
        </span>
        <button
          type="button"
          :class="[ICON, tone(!!current?.liked), empty && OFF]"
          :disabled="empty"
          :aria-label="current?.liked ? 'Remove from liked' : 'Add to liked'"
          @click="like(Number(current!.id), !current!.liked)"
        >
          <LikeHeart :liked="!!current?.liked" />
        </button>
      </div>

      <!-- Where it is. -->
      <div class="flex items-center gap-2 text-xs text-muted-foreground tabular-nums" :class="empty && OFF" :inert="empty || undefined">
        <span class="w-8 text-right">{{ time(empty ? 0 : position) }}</span>
        <Slider class="flex-1" :value="empty ? 0 : position" :max="state?.duration || 1" label="Seek" @change="seek" />
        <span class="w-8">{{ time(empty ? 0 : (state?.duration ?? 0)) }}</span>
      </div>

      <!-- The player. -->
      <div class="flex items-center justify-between">
        <button type="button" :class="[ICON, tone(!!state?.shuffle)]" :aria-pressed="!!state?.shuffle" aria-label="Shuffle" @click="toggleShuffle">
          <Shuffle />
          <span v-if="state?.shuffle" class="absolute bottom-0.5 size-1 rounded-full bg-current" />
        </button>
        <button type="button" :class="[ICON, tone(true), empty && OFF]" :disabled="empty" aria-label="Previous" @click="previous"><PreviousIcon /></button>
        <button
          type="button"
          class="grid size-10 place-items-center rounded-full bg-foreground text-background transition-transform active:scale-95"
          :class="empty && OFF"
          :disabled="empty"
          :aria-label="state?.isPlaying ? 'Pause' : 'Play'"
          @click="togglePlay"
        >
          <PauseIcon v-if="state?.isPlaying" class="size-4" />
          <PlayIcon v-else class="size-4 translate-x-px" />
        </button>
        <button type="button" :class="[ICON, tone(true), empty && OFF]" :disabled="empty" aria-label="Next" @click="next"><NextIcon /></button>
        <button type="button" :class="[ICON, tone(!!state && state.repeatMode !== 'off')]" :aria-label="`Repeat: ${state?.repeatMode ?? 'off'}`" @click="cycleRepeat">
          <Repeat />
          <span v-if="state?.repeatMode === 'one'" class="absolute top-0.5 right-0.5 text-[9px] leading-none font-semibold">1</span>
          <span v-if="state && state.repeatMode !== 'off'" class="absolute bottom-0.5 size-1 rounded-full bg-current" />
        </button>
      </div>

      <!-- The volume. -->
      <div class="flex items-center gap-2 text-muted-foreground">
        <component :is="VolumeIcon" class="size-4 flex-none" />
        <Slider class="flex-1" :value="volume * 100" :max="100" :ticks="volumeSteps || undefined" label="Volume" @live="(v) => setVolume(v / 100)" @change="(v) => setVolume(v / 100)" />
      </div>

      <!-- Melue itself. -->
      <div class="flex items-center justify-between border-t pt-3">
        <button type="button" class="flex items-center gap-2 rounded-md px-2 py-1 text-sm hover:bg-well" @click="invoke('tray_open_main')">
          <ListMusic class="size-4" />Open Melue
        </button>
        <button type="button" class="rounded-md px-2 py-1 text-sm text-muted-foreground hover:bg-well hover:text-foreground" @click="invoke('tray_quit')">Quit</button>
      </div>
    </div>
  </div>
</template>
