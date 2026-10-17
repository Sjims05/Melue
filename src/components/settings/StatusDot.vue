<script setup lang="ts">
/** A status's dot: `ok` (green), `off` (blue: not set up), `bad` (red), `busy` (amber, pulsing: working on it). */
defineProps<{ kind: 'ok' | 'off' | 'bad' | 'busy' }>()
</script>

<template>
  <span
    class="size-2.5 flex-none rounded-full"
    :class="{ 'bg-ok': kind === 'ok', 'bg-off': kind === 'off', 'bg-destructive': kind === 'bad', 'animate-pulse bg-warn': kind === 'busy' }"
  />
</template>
