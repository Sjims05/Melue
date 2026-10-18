<script setup lang="ts">
/**
 * The heading of a page that is a list to play (an album, a folder, a playlist; Melue's design): its big cover on the
 * left; at its right, badges (the `badges` slot), the title, a grey line (the `line` slot: "Album · Juniper Vale ·
 * 2023 · 12 songs · 48 min", with links), and Play and Shuffle.
 */
import { Shuffle } from '@lucide/vue'
import type { Component } from 'vue'
import { Cover, type CoverColors } from '@/components/covers'
import { PlayIcon } from '@/components/player/icons'
import { Button } from '@/components/ui/button'

defineProps<{
  title: string
  image?: string
  colors?: CoverColors
  icon?: Component
  collage?: readonly (string | CoverColors)[]
  /** Nothing to play (an empty list): Play and Shuffle are off. */
  empty?: boolean
}>()
const emit = defineEmits<{ play: []; shuffle: [] }>()
</script>

<template>
  <div class="mb-10 flex items-center gap-12 pt-2">
    <div class="w-[min(16.5rem,30%)] flex-none"><Cover :image="image" :colors="colors" :icon="icon" :collage="collage" /></div>
    <div class="min-w-0">
      <div v-if="$slots.badges" class="mb-4 flex flex-wrap gap-2"><slot name="badges" /></div>
      <h1 class="line-clamp-2 text-4xl leading-tight font-bold tracking-tight">{{ title }}</h1>
      <p class="mt-2 text-lg text-muted-foreground"><slot name="line" /></p>
      <div class="mt-6 flex gap-4">
        <Button size="lg" :disabled="empty" @click="emit('play')"><PlayIcon class="size-3.5" />Play</Button>
        <Button size="lg" variant="outline" :disabled="empty" @click="emit('shuffle')"><Shuffle />Shuffle</Button>
      </div>
    </div>
  </div>
</template>
