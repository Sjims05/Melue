<script setup lang="ts">
/**
 * The phone's mini player, above the bottom bar while something is in the player: the cover, the song, play / pause and
 * next, with a thin line along its top showing how far the song is. Tapping it opens Now playing. It opens and closes
 * smoothly (its height), showing the last song while it closes (the player cleared).
 */
import { Music } from '@lucide/vue'
import { computed, shallowRef, watch } from 'vue'
import { NextIcon, PauseIcon, PlayIcon } from '@/components/player/icons'
import { useSlide } from '@/components/player/slide'
import { PlayingBars } from '@/components/songs'
import { current, next, nowPlaying, position, togglePlay } from '@/player/nowPlaying'

// The song shown: the playing one, or the last one while the bar closes.
const song = shallowRef(current.value)
watch(current, (now) => now && (song.value = now))
const done = computed(() => (nowPlaying.state?.duration ? position.value / nowPlaying.state.duration : 0))
// A jump (back from the background, another device's position): the line slides there.
const sliding = useSlide(done, () => 1)
</script>

<template>
  <div
    class="grid flex-none transition-[grid-template-rows] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]"
    :class="current ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'"
  >
    <div class="min-h-0 overflow-hidden">
      <div v-if="song" class="relative border-t bg-background">
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
    </div>
  </div>
</template>
