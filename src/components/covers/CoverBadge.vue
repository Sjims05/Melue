<script setup lang="ts">
/**
 * A small chip on a cover's corner, solid so it reads on any picture (white like a card in light mode, the page's
 * colour in dark): what kind of playlist it is (Synced, Only on this
 * device, Linked). Just its icon; its word slides out when the card it's on is hovered (or `open`).
 */
import type { Component } from 'vue'

// `also`: a second icon after the first (a linked playlist's link, after where it lives).
defineProps<{ icon: Component; also?: Component; label: string; open?: boolean }>()
</script>

<template>
  <span
    class="absolute top-2.5 right-2.5 flex h-[26px] items-center rounded-md bg-card px-[5px] text-foreground shadow-sm dark:bg-background"
    :aria-label="label"
  >
    <component :is="icon" class="size-4 flex-none" :stroke-width="1.75" />
    <component :is="also" v-if="also" class="ml-1 size-4 flex-none" :stroke-width="1.75" />
    <span
      class="grid transition-[grid-template-columns,opacity] duration-300 ease-[cubic-bezier(.2,.8,.2,1)] motion-reduce:transition-none"
      :class="open ? 'grid-cols-[1fr] opacity-100' : 'grid-cols-[0fr] opacity-0 group-hover/cover:grid-cols-[1fr] group-hover/cover:opacity-100 group-focus-visible/cover:grid-cols-[1fr] group-focus-visible/cover:opacity-100'"
    >
      <span class="overflow-hidden text-xs font-medium whitespace-nowrap"><span class="pl-1.5">{{ label }}</span></span>
    </span>
  </span>
</template>
