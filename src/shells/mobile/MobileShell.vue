<script setup lang="ts">
/**
 * The phone app's frame: its own navigation (hotbar, top words, mini player, player sheet) will live here,
 * around the shared pages. For now it shows the spike: its tests, and the finished components.
 */
import MobileSpike from '@/spike/MobileSpike.vue'
import SpikeFrame from '@/spike/SpikeFrame.vue'
</script>

<template>
  <SpikeFrame>
    <MobileSpike />
  </SpikeFrame>
</template>
