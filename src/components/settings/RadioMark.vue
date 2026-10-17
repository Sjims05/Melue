<script setup lang="ts">
/**
 * A radio's circle, for a row that's one of a list (the output devices): its ring thickens into the picked look when
 * it's chosen, and thins back when another one is.
 */
defineProps<{ on: boolean }>()
</script>

<template>
  <span
    class="size-[18px] flex-none rounded-full transition-[border-width,border-color,opacity] duration-200 ease-[cubic-bezier(.2,.8,.2,1)] motion-reduce:transition-none"
    :class="on ? 'border-[5px] border-foreground' : 'border-[1.5px] border-muted-foreground opacity-60'"
  />
</template>
