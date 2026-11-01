<script setup lang="ts">
/**
 * A status's dot: `ok` (green), `streamed` (purple: fine, played straight from the share), `off` (blue: not set up),
 * `bad` (red), `busy` (amber, pulsing: working on it).
 */
defineProps<{ kind: 'ok' | 'streamed' | 'off' | 'bad' | 'busy' }>()
</script>

<template>
  <span
    class="size-2.5 flex-none rounded-full"
    :class="{ 'bg-ok': kind === 'ok', 'bg-streamed': kind === 'streamed', 'bg-off': kind === 'off', 'bg-destructive': kind === 'bad', 'animate-pulse bg-warn': kind === 'busy' }"
  />
</template>
