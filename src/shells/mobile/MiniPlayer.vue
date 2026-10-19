<script setup lang="ts">
/**
 * The phone's mini player, above the bottom bar while something is in the player: the cover, the song, play / pause and
 * next, with a thin line along its top showing how far the song is. Tapping it opens Now playing.
 */
import { Music } from '@lucide/vue'
import { computed } from 'vue'
import { NextIcon, PauseIcon, PlayIcon } from '@/components/player/icons'
import { useSlide } from '@/components/player/slide'
import { PlayingBars } from '@/components/songs'
import { current, next, nowPlaying, position, togglePlay } from '@/player/nowPlaying'

const done = computed(() => (nowPlaying.state?.duration ? position.value / nowPlaying.state.duration : 0))
// A jump (back from the background, another device's position): the line slides there.
const sliding = useSlide(done, () => 1)
</script>

<template>
  <div v-if="current" class="relative flex-none border-t bg-background">
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
            <img v-if="current.cover" :key="current.cover" :src="current.cover" alt="" class="absolute inset-0 size-full object-cover" draggable="false" />
          </Transition>
          <Music v-if="!current.cover" class="size-5" />
        </span>
        <Transition name="swap" mode="out-in">
          <span :key="current.id" class="flex min-w-0 flex-col">
            <span class="truncate text-[15px] font-semibold">{{ current.title }}</span>
            <span class="truncate text-[13px] text-muted-foreground">{{ current.artist }}</span>
          </span>
        </Transition>
        <PlayingBars :paused="!nowPlaying.state?.isPlaying" class="ml-1 flex-none scale-125" />
      </RouterLink>
      <button
        type="button"
        class="grid size-11 flex-none place-items-center rounded-full transition-transform active:scale-90 [&_svg]:size-5"
        :aria-label="nowPlaying.state?.isPlaying ? 'Pause' : 'Play'"
        @click="togglePlay"
      >
        <PauseIcon v-if="nowPlaying.state?.isPlaying" />
        <PlayIcon v-else class="translate-x-px" />
      </button>
      <button type="button" class="grid size-11 flex-none place-items-center rounded-full transition-transform active:scale-90 [&_svg]:size-5" aria-label="Next" @click="next">
        <NextIcon />
      </button>
    </div>
  </div>
</template>
