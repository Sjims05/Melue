<script setup lang="ts">
/**
 * Library's Artists: a row per artist (their name, "12 songs"), in the chosen order. A row opens the artist; its
 * right-click menu plays all their songs, shuffles them, plays them next or adds them to the queue. It follows the
 * library.
 */
import { CircleAlert, User } from '@lucide/vue'
import { onUnmounted, ref, watch } from 'vue'
import { coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, LinkRow } from '@/components/page'
import { library, type Artist, type ArtistSort } from '@/library'
import { count } from '@/lib/format'
import { listAction } from '@/player/songs'
import { useRouter } from 'vue-router'
import { Button } from '@/components/ui/button'

const props = defineProps<{ sort: ArtistSort }>()

const artists = ref<Artist[] | null>(null)
// Why the list couldn't be read (shown instead of an empty list).
const problem = ref('')
const router = useRouter()
async function load() {
  problem.value = ''
  artists.value = await library.artists(0, 1_000_000, props.sort).catch((e) => ((problem.value = String(e)), []))
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
    <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
    <Transition name="part" mode="out-in">
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
      <!-- Can't be read (Try again), or nothing yet (where to add music). -->
      <EmptyState v-else-if="problem" :icon="CircleAlert" title="Can’t read your library" :text="problem" problem>
        <Button size="sm" variant="outline" @click="load">Try again</Button>
      </EmptyState>
      <EmptyState v-else-if="artists" :icon="User" title="No artists yet" text="Songs with an artist in their tags show up here as artists.">
        <Button size="sm" @click="router.push('/settings')">Add a music folder</Button>
      </EmptyState>
    </Transition>
  </div>
</template>
