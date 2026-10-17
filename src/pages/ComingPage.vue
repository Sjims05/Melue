<script setup lang="ts">
/** A page opened on top of another (an artist, an album, a folder, a playlist), until its step builds it. */
import { useRoute } from 'vue-router'
import { PageHeader, PlannedCard } from '@/components/page'

const route = useRoute()
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader :title="String(route.meta.title)" />
    <PlannedCard :title="`${route.meta.title} page`" text="Built in its step: Library (album, artist, folder) or Playlists (a playlist)." />
  </div>
</template>
