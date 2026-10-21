<script setup lang="ts">
/**
 * Library's Albums: a grid of album cards (cover, title, album artist), in the chosen order. A card opens its album;
 * its right-click menu plays it, shuffles it, plays it next, adds it to the queue, or goes to its artist. It follows
 * the library.
 */
import { CircleAlert, Disc3 } from '@lucide/vue'
import { onUnmounted, ref, watch } from 'vue'
import { CoverCard, coverMenu, type CoverAction } from '@/components/covers'
import { coverUrl, library, type Album, type AlbumSort } from '@/library'
import { listAction } from '@/player/songs'
import { useRouter } from 'vue-router'
import { EmptyState } from '@/components/page'
import { Button } from '@/components/ui/button'

const props = defineProps<{ sort: AlbumSort }>()
const router = useRouter()

const albums = ref<Album[] | null>(null)
// Why the list couldn't be read (shown instead of an empty list).
const problem = ref('')
async function load() {
  problem.value = ''
  albums.value = await library.albums(0, 1_000_000, props.sort).catch((e) => ((problem.value = String(e)), []))
}
watch(() => props.sort, load, { immediate: true })
const stop = library.onChange((change) => change.reason !== 'likes' && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const MENU = coverMenu({ kind: 'album' })
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
    <Transition name="part" mode="out-in">
      <div v-if="albums?.length" class="cover-grid [--cover-min:10.5rem]">
        <CoverCard
          v-for="album in albums"
          :key="album.id"
          :title="album.title"
          :subtitle="album.artist ?? 'Various artists'"
          :image="album.cover ? coverUrl(album.cover) : undefined"
          :menu="MENU"
          @open="router.push(`/album/${album.id}`)"
          @action="(action) => listAction(`album:${album.id}`, action as CoverAction, album.artistId)"
        />
      </div>
      <!-- Can't be read (Try again), or nothing yet (where to add music). -->
      <EmptyState v-else-if="problem" :icon="CircleAlert" title="Can’t read your library" :text="problem" problem>
        <Button size="sm" variant="outline" @click="load">Try again</Button>
      </EmptyState>
      <EmptyState v-else-if="albums" :icon="Disc3" title="No albums yet" text="Songs with an album in their tags show up here as albums.">
        <Button size="sm" @click="router.push('/settings')">Add a music folder</Button>
      </EmptyState>
    </Transition>
  </div>
</template>
