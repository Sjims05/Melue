<script setup lang="ts">
/**
 * The phone's mini player, above the bottom bar while something is in the player: the cover, the song, play / pause and
 * next, with a thin line along its top showing how far the song is. Tapping it opens Now playing. It opens and closes
 * smoothly (its height), showing the last song while it closes (the player cleared). A long press opens its menu:
 * Connect device (play on another of your devices, or here again), and Clear the player (the computer's player bar has
 * an ✕ for it). Nothing loaded: it stays, empty ("Nothing playing"),
 * like a computer's player bar, unless Display's "Hide the player when nothing is loaded" is on.
 */
import { MonitorSpeaker, Music, X } from '@lucide/vue'
import { computed, shallowRef, watch } from 'vue'
import { NextIcon, PauseIcon, PlayIcon } from '@/components/player/icons'
import { useSlide } from '@/components/player/slide'
import { PlayingBars } from '@/components/songs'
import { clearPlayer, current, next, nothingHere, nowPlaying, position, togglePlay } from '@/player/nowPlaying'
import { useLongPress } from '@/lib/longPress'
import { appearance } from '@/lib/appearance'
import { openMenu } from '@/lib/menuDrawer'
import { openDevicesDrawer } from '@/player/remotePlayback'
import { connectedTo } from '@/player/playingOn'

// The song shown: the playing one, or the last one while the bar closes.
const song = shallowRef(current.value)
watch(current, (now) => now && (song.value = now))
// Nothing loaded: shown empty, unless it's set to hide then (Display settings).
const shown = computed(() => !!current.value || !appearance.hideEmptyMiniPlayer)
const empty = computed(() => !current.value && !appearance.hideEmptyMiniPlayer)
const done = computed(() => (nowPlaying.state?.duration ? position.value / nowPlaying.state.duration : 0))
// A jump (back from the background, another device's position): the line slides there.
const sliding = useSlide(done, () => 1)

// A long press: the player's menu, with the song at its top (nothing loaded: Connect device only).
const press = useLongPress(() => {
  const playing = current.value
  openMenu({
    title: playing?.title ?? 'Nothing playing',
    subtitle: playing?.artist ?? undefined,
    cover: { image: playing?.cover ?? undefined, placeholder: Music },
    items: [
      // As Now playing's button: the device it's connected to, while it is.
      { action: 'devices', label: connectedTo() ?? 'Connect device', icon: MonitorSpeaker },
      ...(playing ? [{ action: 'clear', label: 'Clear the player', icon: X, danger: true, apart: true }] : []),
    ],
    pick: (action) => (action === 'clear' ? clearPlayer() : action === 'devices' ? openDevicesDrawer() : undefined),
  })
})
</script>

<template>
  <div
    class="grid flex-none transition-[grid-template-rows] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]"
    :class="shown ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'"
  >
    <div class="min-h-0 overflow-hidden">
      <!-- Nothing loaded (and set to stay): the bar, empty. Tapping still opens Now playing. -->
      <div v-if="empty" class="relative border-t bg-background" v-bind="press">
        <div class="absolute inset-x-0 top-0 h-0.5 bg-muted-foreground/20" />
        <div class="flex h-16 items-center gap-3 pr-2 pl-4">
          <RouterLink to="/now-playing" class="flex min-w-0 flex-1 items-center gap-3" aria-label="Open Now playing">
            <span class="grid size-11 flex-none place-items-center rounded-md bg-well text-muted-foreground"><Music class="size-5" /></span>
            <span class="flex min-w-0 flex-col">
              <span class="truncate text-[15px] font-semibold">Nothing playing</span>
              <span class="truncate text-[13px] text-muted-foreground">Play a song, an album or a playlist</span>
            </span>
          </RouterLink>
          <button type="button" class="grid size-11 flex-none place-items-center rounded-full text-muted-foreground/50 [&_svg]:size-5" aria-label="Play" disabled>
            <PlayIcon class="translate-x-px" />
          </button>
          <button type="button" class="grid size-11 flex-none place-items-center rounded-full text-muted-foreground/50 [&_svg]:size-5" aria-label="Next" disabled>
            <NextIcon />
          </button>
        </div>
      </div>
      <div v-else-if="song" class="relative border-t bg-background" v-bind="press">
        <div class="absolute inset-x-0 top-0 h-0.5 bg-muted-foreground/20">
          <div
            class="h-full bg-foreground"
            :class="sliding && 'transition-[width] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]'"
            :style="{ width: `${done * 100}%` }"
          />
        </div>
        <div class="flex h-16 items-center gap-3 pr-2 pl-4">
          <RouterLink to="/now-playing" class="flex min-w-0 flex-1 items-center gap-3" aria-label="Open Now playing">
            <span class="relative grid size-11 flex-none place-items-center overflow-hidden rounded-md bg-well text-muted-foreground">
              <Transition name="fade">
                <img v-if="song.cover" :key="song.cover" :src="song.cover" alt="" class="absolute inset-0 size-full object-cover" draggable="false" />
              </Transition>
              <Music v-if="!song.cover" class="size-5" />
            </span>
            <Transition name="swap" mode="out-in">
              <span :key="song.id" class="flex min-w-0 flex-col">
                <span class="truncate text-[15px] font-semibold">{{ song.title }}</span>
                <span class="truncate text-[13px] text-muted-foreground">{{ song.artist }}</span>
              </span>
            </Transition>
            <PlayingBars :paused="!nowPlaying.state?.isPlaying" class="ml-1 flex-none scale-125" />
          </RouterLink>
          <button
            type="button"
            class="grid size-11 flex-none place-items-center rounded-full transition-transform active:scale-90 disabled:opacity-40 [&_svg]:size-5"
            :disabled="nothingHere"
            :aria-label="nowPlaying.state?.isPlaying ? 'Pause' : 'Play'"
            @click="togglePlay"
          >
            <PauseIcon v-if="nowPlaying.state?.isPlaying" />
            <PlayIcon v-else class="translate-x-px" />
          </button>
          <button
            type="button"
            class="grid size-11 flex-none place-items-center rounded-full transition-transform active:scale-90 disabled:opacity-40 [&_svg]:size-5"
            :disabled="nothingHere"
            aria-label="Next"
            @click="next"
          >
            <NextIcon />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
