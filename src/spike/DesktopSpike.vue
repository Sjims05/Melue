<script setup lang="ts">
/**
 * Spike tests for Windows / macOS / Linux: the shared sections (SpikePage) and playback (PlaybackSpike),
 * through the native-audio plugin's desktop player. The Android Auto and car sections don't apply.
 */
import PlaybackSpike from './PlaybackSpike.vue'
import SpikePage from './SpikePage.vue'
</script>

<template>
  <SpikePage title="Spike Tests (desktop)">
    <PlaybackSpike />
  </SpikePage>
</template>
