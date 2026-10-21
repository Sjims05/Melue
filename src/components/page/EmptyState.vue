<script setup lang="ts">
/**
 * What a page or a part of one shows when there's nothing (no songs yet, no results) or something went wrong (the
 * library can't be read): an icon in a soft circle, a short title, a line of help, and the action that helps (the
 * default slot: buttons). `small`: inside a section of a page (left-aligned, less room around it); otherwise centred
 * on the page. `problem`: something went wrong (the icon in red).
 */
import type { Component } from 'vue'

defineProps<{ icon: Component; title: string; text?: string; small?: boolean; problem?: boolean }>()
</script>

<template>
  <div class="flex flex-col gap-3" :class="small ? 'items-start py-2' : 'items-center py-14 text-center phone:py-10'" role="status">
    <span class="grid flex-none place-items-center rounded-full bg-well" :class="[small ? 'size-11' : 'size-16', problem ? 'text-destructive' : 'text-muted-foreground']">
      <component :is="icon" :class="small ? 'size-5' : 'size-7'" :stroke-width="1.75" />
    </span>
    <div class="flex max-w-md flex-col gap-1">
      <p class="font-semibold" :class="small ? 'text-[15px]' : 'text-lg'">{{ title }}</p>
      <p v-if="text" class="text-muted-foreground" :class="small ? 'text-sm' : 'text-[15px]'">{{ text }}</p>
    </div>
    <div v-if="$slots.default" class="mt-1 flex flex-wrap gap-2" :class="!small && 'justify-center'"><slot /></div>
  </div>
</template>
