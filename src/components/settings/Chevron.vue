<script setup lang="ts">
/**
 * A chevron that turns when what it belongs to opens (the standard for everything that opens): one pointing down
 * turns up, one pointing right turns down.
 */
import { ChevronDown, ChevronRight } from '@lucide/vue'

withDefaults(defineProps<{ open: boolean; points?: 'down' | 'right' }>(), { points: 'down' })
</script>

<template>
  <component
    :is="points === 'down' ? ChevronDown : ChevronRight"
    class="size-[18px] text-foreground transition-transform duration-300 ease-[cubic-bezier(.2,.8,.2,1)] motion-reduce:transition-none"
    :class="open && (points === 'down' ? 'rotate-180' : 'rotate-90')"
  />
</template>
