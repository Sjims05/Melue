<script setup lang="ts">
/**
 * A radio's circle, for a row that's one of a list (the output devices): its ring thickens into the picked look when
 * it's chosen, and thins back when another one is. The ring is an inner shadow, not a border: borders snap to whole
 * pixels while they animate (it jumps), shadows don't.
 */
defineProps<{ on: boolean }>()
</script>

<template>
  <span
    class="size-[18px] flex-none rounded-full transition-[box-shadow,opacity] duration-200 ease-[cubic-bezier(.2,.8,.2,1)] motion-reduce:transition-none"
    :class="on ? 'shadow-[inset_0_0_0_5px_var(--foreground)]' : 'shadow-[inset_0_0_0_1.5px_var(--muted-foreground)] opacity-60'"
  />
</template>
