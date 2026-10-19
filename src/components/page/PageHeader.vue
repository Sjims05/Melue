<script setup lang="ts">
/**
 * A main page's heading (Melue's design): a big title and a grey line under it ("44 songs · 8 albums · 6 artists").
 * The `actions` slot sits at its right (an artist's Play and Shuffle); the default slot under it (Library's tabs).
 */
defineProps<{ title: string; subtitle?: string }>()
</script>

<template>
  <div class="mb-8 flex flex-wrap items-start justify-between gap-x-6 gap-y-4 pt-2 phone:mb-5 phone:gap-y-3 phone:pt-1">
    <div class="min-w-0">
      <h1 class="truncate text-5xl leading-tight font-bold tracking-tight phone:text-[32px]">{{ title }}</h1>
      <p v-if="subtitle" class="mt-1 text-lg text-muted-foreground phone:mt-0 phone:text-[15px]">{{ subtitle }}</p>
    </div>
    <div v-if="$slots.actions" class="flex items-center gap-4 pt-3"><slot name="actions" /></div>
    <div v-if="$slots.default" class="w-full"><slot /></div>
  </div>
</template>
