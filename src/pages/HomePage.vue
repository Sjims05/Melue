<script setup lang="ts">
/** Home: what's playing and what another device was playing, recently played lists, recent songs (Melue's design). */
import { computed } from 'vue'
import { PageHeader, PlannedCard } from '@/components/page'

// The grey line: the time of day, as in the design ("Late night").
const part = computed(() => {
  const h = new Date().getHours()
  return h < 5 ? 'Late night' : h < 12 ? 'Morning' : h < 17 ? 'Afternoon' : h < 22 ? 'Evening' : 'Late night'
})
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader title="Home" :subtitle="part" />
    <PlannedCard title="Home" text="Playing now, continue from another device, recently played and recent songs: a coming step (Home and Search)." />
  </div>
</template>
