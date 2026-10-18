<script setup lang="ts">
/**
 * Library's Albums: a grid of album cards (cover, title, album artist), in the chosen order. A card opens its album;
 * its right-click menu plays it, shuffles it, plays it next, adds it to the queue, or goes to its artist. It follows
 * the library.
 */
import { onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { CoverCard, coverMenu, type CoverAction } from '@/components/covers'
import { coverUrl, library, type Album, type AlbumSort } from '@/library'
import { listAction } from '@/player/songs'

const props = defineProps<{ sort: AlbumSort }>()
const router = useRouter()

const albums = ref<Album[] | null>(null)
async function load() {
  albums.value = await library.albums(0, 1_000_000, props.sort).catch(() => [])
}
watch(() => props.sort, load, { immediate: true })
const stop = library.onChange((change) => change.reason !== 'likes' && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const MENU = coverMenu({ kind: 'album' })
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
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
    <p v-else-if="albums" class="px-2 text-muted-foreground">No albums yet: songs with an album tag show up here.</p>
  </div>
</template>
