<script setup lang="ts">
/** Dev builds only: the spike's pages (Tests, Settings, Components), as before the real pages. */
import { isMobile } from '@/platform'
import DesktopSpike from '@/spike/DesktopSpike.vue'
import MobileSpike from '@/spike/MobileSpike.vue'
import SpikeFrame from '@/spike/SpikeFrame.vue'
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <SpikeFrame>
      <MobileSpike v-if="isMobile" />
      <DesktopSpike v-else />
    </SpikeFrame>
  </div>
</template>
