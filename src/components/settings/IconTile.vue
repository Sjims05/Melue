<script setup lang="ts">
/** A row's leading icon on a small rounded tile (a folder, the share's cloud, a network drive). */
import type { Component } from 'vue'

defineProps<{ icon: Component }>()
</script>

<template>
  <span class="grid size-[34px] flex-none place-items-center rounded-lg bg-well shadow-[inset_0_0_0_1px_var(--well-line)] text-foreground">
    <component :is="icon" class="size-4" />
  </span>
</template>
