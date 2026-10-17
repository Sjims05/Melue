<script setup lang="ts">
/** Playlists: yours (synced, only on this device, or linked) and Made for you (the auto playlists). */
import { PageHeader, PlannedCard } from '@/components/page'
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader title="Playlists" subtitle="Yours, and made for you" />
    <PlannedCard title="Playlists" text="Your playlists and Made for you, a playlist's page, new playlist, rename, delete, hide: a coming step (Playlists)." />
  </div>
</template>
