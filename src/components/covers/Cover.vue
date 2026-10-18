<script setup lang="ts">
/**
 * A cover, square with rounded corners: a picture (`image`), the Cover maker's gradient (`colors`, with an `icon` in
 * the middle for an auto playlist), or a 2×2 collage (`collage`: four pictures or gradients, a playlist's songs).
 * Nothing of those: the note, as on a song without a cover. Its slot sits on top (a CoverBadge in the corner).
 */
import { Music } from '@lucide/vue'
import type { Component } from 'vue'
import { coverCss, type CoverColors } from './coverRecipe'

defineProps<{
  image?: string
  colors?: CoverColors
  icon?: Component
  collage?: readonly (string | CoverColors)[]
}>()
</script>

<template>
  <div class="relative aspect-square w-full overflow-hidden rounded-lg bg-well">
    <div v-if="collage?.length" class="grid size-full grid-cols-2 grid-rows-2">
      <template v-for="(tile, i) in collage.slice(0, 4)" :key="i">
        <img v-if="typeof tile === 'string'" :src="tile" alt="" class="size-full object-cover" draggable="false" />
        <div v-else class="size-full" :style="{ background: coverCss(tile) }" />
      </template>
    </div>
    <img v-else-if="image" :src="image" alt="" class="size-full object-cover" loading="lazy" draggable="false" />
    <div v-else-if="colors" class="grid size-full place-items-center" :style="{ background: coverCss(colors) }">
      <component :is="icon" v-if="icon" class="size-[24%] text-white/92" :stroke-width="1.75" />
    </div>
    <div v-else class="grid size-full place-items-center text-muted-foreground"><Music class="size-[24%]" :stroke-width="1.5" /></div>
    <slot />
  </div>
</template>
