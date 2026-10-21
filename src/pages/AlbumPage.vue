<script setup lang="ts">
/**
 * An album (Melue's design, opened on top): its cover, title and line ("Album · Juniper Vale · 2023 · 12 songs ·
 * 48 min", the artist linking to their page), Play and Shuffle; then its songs in order, with their track numbers
 * (no covers: they're all the album's), under "Disc 1", "Disc 2" when it has more than one. It follows the library.
 */
import { computed, onUnmounted, ref } from 'vue'
import { useRoute, type RouteLocationNormalized } from 'vue-router'
import { EmptyState, ListHeader } from '@/components/page'
import { SongList, type Song } from '@/components/songs'
import { coverUrl, library, type AlbumPage, type Source } from '@/library'
import { count, totalLength } from '@/lib/format'
import { isDesktop } from '@/platform/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { coverMenu, type CoverAction } from '@/components/covers'
import { listAction, playList, playSong, songAction, trackSong } from '@/player/songs'
import { goBack, setPageTitle } from '@/router'
import { Disc3 } from '@lucide/vue'
import { Button } from '@/components/ui/button'

const route = useRoute()
// This page's own address and album (the route moves on when another page opens on top of it).
const here = { fullPath: route.fullPath } as RouteLocationNormalized
const id = Number(route.params.id)
const source: Source = `album:${id}`

// undefined while it loads; null: no such album (any more).
const page = ref<AlbumPage | null>()
async function load() {
  page.value = await library.albumPage(id).catch(() => null)
  if (page.value) setPageTitle(here, page.value.album.title)
}
load()
const stop = library.onChange((change) => change.sources.includes(source) && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const album = computed(() => page.value?.album)
const songs = computed(() => page.value?.tracks.map(trackSong) ?? [])
const tracks = computed(() => new Map(page.value?.tracks.map((t) => [t.id, t])))
const length = computed(() => page.value?.tracks.reduce((sum, t) => sum + (t.durationMs ?? 0), 0) ?? 0)
// "Disc 2" headings, when it has more than one disc.
const discOf = (song: Song) => tracks.value.get(Number(song.id))?.discNo ?? 1
const headingOf = computed(() => (new Set(songs.value.map(discOf)).size > 1 ? (s: Song) => `Disc ${discOf(s)}` : undefined))
const numberOf = (song: Song, index: number) => tracks.value.get(Number(song.id))?.trackNo ?? index + 1

// The heading's right-click menu: the album card's.
const MENU = coverMenu({ kind: 'album' })

</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <template v-if="album">
      <ListHeader
        :title="album.title"
        :image="album.cover ? coverUrl(album.cover, 'large') : undefined"
        :empty="!songs.length"
        :menu="MENU"
        @action="(action) => listAction(source, action as CoverAction, album!.artistId)"
        @play="playList(source)"
        @shuffle="playList(source, { shuffle: true })"
      >
        <template #line>
          Album ·
          <RouterLink v-if="album.artistId != null" :to="`/artist/${album.artistId}`" class="hover:text-foreground hover:underline">{{ album.artist }}</RouterLink>
          <template v-else>{{ album.artist ?? 'Various artists' }}</template>
          <template v-if="album.year"> · {{ album.year }}</template>
          · {{ count(songs.length, 'song') }} · {{ totalLength(length) }}
        </template>
      </ListHeader>
      <SongList
        :songs="songs"
        :heading-of="headingOf"
        :number-of="numberOf"
        album-page
        :current-id="current?.id"
        :playing="!!nowPlaying.state?.isPlaying"
        :place="{ showInFolder: isDesktop }"
        @play="playSong"
        @pause="togglePlay"
        @like="(song, liked) => like(Number(song.id), liked)"
        @action="songAction"
      />
    </template>
    <EmptyState v-else-if="page === null" :icon="Disc3" title="This album isn’t in your library" text="Its songs may have been removed, or their folder switched off.">
      <Button size="sm" variant="outline" @click="goBack">Back</Button>
    </EmptyState>
  </div>
</template>
