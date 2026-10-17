<script setup lang="ts">
/**
 * The desktop app's frame: its own navigation (side dots, top search, player bar) will live here,
 * around the shared pages. For now it shows the spike: its tests, and the finished components.
 */
import DesktopSpike from '@/spike/DesktopSpike.vue'
import SpikeFrame from '@/spike/SpikeFrame.vue'
</script>

<template>
  <SpikeFrame>
    <DesktopSpike />
  </SpikeFrame>
</template>
