<script setup lang="ts">
/**
 * Spike tests for Windows / macOS / Linux: the shared sections (SpikePage). Playback (sections 3–4)
 * waits for the desktop player; the Android Auto and car sections don't apply to desktop.
 */
import SpikePage from './SpikePage.vue'
</script>

<template>
  <SpikePage title="Spike Tests (desktop)">
    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">3–4. Playback and queue</h2>
      <p class="text-sm text-muted-foreground">The desktop player isn't built yet.</p>
    </section>
  </SpikePage>
</template>
