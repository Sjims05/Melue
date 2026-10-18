<script setup lang="ts">
/**
 * Library's Artists: a row per artist (their name, "12 songs"), in the chosen order. A row opens the artist; its
 * right-click menu plays all their songs, shuffles them, plays them next or adds them to the queue. It follows the
 * library.
 */
import { onUnmounted, ref, watch } from 'vue'
import { coverMenu, type CoverAction } from '@/components/covers'
import { LinkRow } from '@/components/page'
import { library, type Artist, type ArtistSort } from '@/library'
import { count } from '@/lib/format'
import { listAction } from '@/player/songs'

const props = defineProps<{ sort: ArtistSort }>()

const artists = ref<Artist[] | null>(null)
async function load() {
  artists.value = await library.artists(0, 1_000_000, props.sort).catch(() => [])
}
watch(() => props.sort, load, { immediate: true })
const stop = library.onChange((change) => change.reason !== 'likes' && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const MENU = coverMenu({ kind: 'list' })
// Their own songs; only featured ones: those.
const line = (a: Artist) => (a.tracks ? count(a.tracks, 'song') : `Featured on ${count(a.featured, 'song')}`)
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <div v-if="artists?.length" role="list">
      <LinkRow
        v-for="artist in artists"
        :key="artist.id"
        role="listitem"
        :to="`/artist/${artist.id}`"
        :title="artist.name"
        :subtitle="line(artist)"
        :menu="MENU"
        @action="(action) => listAction(`artist:${artist.id}`, action as CoverAction)"
      />
    </div>
    <p v-else-if="artists" class="px-2 text-muted-foreground">No artists yet: songs with an artist tag show up here.</p>
  </div>
</template>
