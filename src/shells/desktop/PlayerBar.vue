<script setup lang="ts">
/**
 * The player bar, at the bottom of every page but Now playing (Melue's design): the song (its cover and words open Now
 * playing) with the playing bars; then, well apart, shuffle, previous, play / pause, next, repeat (on: white with a dot
 * under it; repeat one: a "1"; the player's own icons, components/player/icons.ts); the seek bar with both times; then
 * the heart, devices (planned: Phase 4), the queue (Now playing with Up next open) and ✕ (stops and empties the
 * player). Always there: with nothing in the queue it's empty ("Nothing playing", the note icon for a cover, as a
 * song that can't play has), only shuffle and repeat on (they're for what's played next).
 */
import LikeHeart from '@/components/player/LikeHeart.vue'
import { ListMusic, MonitorSpeaker, Music, Repeat, Shuffle, X } from '@lucide/vue'
import { NextIcon, PauseIcon, PlayIcon, PreviousIcon } from '@/components/player/icons'
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import Slider from '@/components/player/Slider.vue'
import { PlayingBars } from '@/components/songs'
import VolumeButton from './VolumeButton.vue'
import { clearPlayer, current, cycleRepeat, like, next, nowPlaying, position, previous, seek, toggleShuffle, togglePlay } from '@/player/nowPlaying'

const router = useRouter()
const state = computed(() => nowPlaying.state)
// Nothing in the queue: the bar stays, empty, with what needs a song off.
const empty = computed(() => !current.value || !state.value)
const time = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

// A button's colour is one class or the other (two colour classes: the stylesheet's order would decide, not this).
const ICON = 'relative grid size-9 place-items-center rounded-md transition-colors hover:text-foreground [&_svg]:size-4'
const tone = (on: boolean) => (on ? 'text-foreground' : 'text-muted-foreground')
const OFF = 'pointer-events-none opacity-40'
</script>

<template>
  <footer class="grid h-[92px] flex-none grid-cols-[360px_auto_minmax(0,1fr)_auto] items-center gap-x-12 border-t px-12">
    <!-- The song. A set width, so the controls stay put: a long title is cut off (the grid's gap keeps it off them). -->
    <RouterLink v-if="current && state" to="/now-playing" class="flex min-w-0 items-center gap-4">
      <span class="relative size-[52px] flex-none overflow-hidden rounded-md bg-well">
        <Transition name="fade">
          <img v-if="current.cover" :key="current.cover" :src="current.cover" alt="" class="absolute inset-0 size-full object-cover" draggable="false" />
        </Transition>
      </span>
      <Transition name="swap" mode="out-in">
        <span :key="current.id" class="flex min-w-0 flex-col">
          <span class="truncate text-[15px] font-semibold">{{ current.title }}</span>
          <span class="truncate text-[15px] text-muted-foreground">{{ current.artist }}</span>
        </span>
      </Transition>
      <PlayingBars :paused="!state.isPlaying" class="ml-1 flex-none scale-150" />
    </RouterLink>
    <div v-else class="flex min-w-0 items-center gap-4">
      <span class="grid size-[52px] flex-none place-items-center rounded-md bg-well text-muted-foreground"><Music class="size-5" /></span>
      <span class="flex min-w-0 flex-col">
        <span class="truncate text-[15px] font-semibold">Nothing playing</span>
        <span class="truncate text-[15px] text-muted-foreground">Play a song to start</span>
      </span>
    </div>

    <!-- The controls. -->
    <div class="flex items-center gap-3">
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
      <button
        type="button"
        :class="[ICON, tone(!!state && state.repeatMode !== 'off')]"
        :aria-label="`Repeat: ${state?.repeatMode ?? 'off'}`"
        @click="cycleRepeat"
      >
        <Repeat />
        <span v-if="state?.repeatMode === 'one'" class="absolute top-0.5 right-0.5 text-[9px] leading-none font-semibold">1</span>
        <span v-if="state && state.repeatMode !== 'off'" class="absolute bottom-0.5 size-1 rounded-full bg-current" />
      </button>
    </div>

    <!-- Where it is. -->
    <div class="flex min-w-0 items-center gap-4 text-sm text-muted-foreground tabular-nums" :class="empty && OFF" :inert="empty || undefined">
      <span class="w-10 text-right">{{ time(empty ? 0 : position) }}</span>
      <Slider class="flex-1" :value="empty ? 0 : position" :max="state?.duration || 1" label="Seek" @change="seek" />
      <span class="w-10">{{ time(empty ? 0 : (state?.duration ?? 0)) }}</span>
    </div>

    <!-- The rest. -->
    <div class="flex items-center gap-2">
      <button
        type="button"
        :class="[ICON, tone(!!current?.liked), empty && OFF]"
        :disabled="empty"
        :aria-label="current?.liked ? 'Remove from liked' : 'Add to liked'"
        @click="like(Number(current!.id), !current!.liked)"
      >
        <LikeHeart :liked="!!current?.liked" />
      </button>
      <button type="button" :class="[ICON, tone(false), 'cursor-default opacity-40 hover:text-muted-foreground']" title="Planned: play on another device (Phase 4)" disabled>
        <MonitorSpeaker />
      </button>
      <VolumeButton :icon-class="[ICON, tone(false)].join(' ')" />
      <button
        type="button"
        :class="[ICON, tone(false), empty && OFF]"
        :disabled="empty"
        aria-label="Queue"
        @click="router.push({ path: '/now-playing', query: { queue: 'open' } })"
      >
        <ListMusic />
      </button>
      <button type="button" :class="[ICON, tone(false), empty && OFF]" :disabled="empty" title="Clear the player" aria-label="Clear the player" @click="clearPlayer">
        <X />
      </button>
    </div>
  </footer>
</template>
