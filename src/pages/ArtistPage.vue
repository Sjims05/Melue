<script setup lang="ts">
/**
 * An artist (Melue's design, opened on top): their name and line ("Artist · 44 songs · 8 albums"), Play and Shuffle
 * at the right (all their songs: their albums oldest first, then the rest); then their Albums (cards, oldest first),
 * their Songs, and the songs they're Featured on. It follows the library.
 */
import { computed, onUnmounted, ref } from 'vue'
import { useRoute, useRouter, type RouteLocationNormalized } from 'vue-router'
import { Shuffle, User } from '@lucide/vue'
import { CoverCard, coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, PageHeader } from '@/components/page'
import { PlayIcon } from '@/components/player/icons'
import { SongList, type Song } from '@/components/songs'
import { Button } from '@/components/ui/button'
import { coverUrl, library, type ArtistPage, type Source } from '@/library'
import { count } from '@/lib/format'
import { isDesktop } from '@/platform/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { listAction, playList, playSong, songAction, trackSong } from '@/player/songs'
import { goBack, setPageTitle } from '@/router'

const route = useRoute()
const router = useRouter()
// This page's own address and artist (the route moves on when another page opens on top of it).
const here = { fullPath: route.fullPath } as RouteLocationNormalized
const id = Number(route.params.id)
const source: Source = `artist:${id}`

// undefined while it loads; null: no such artist (any more).
const page = ref<ArtistPage | null>()
async function load() {
  page.value = await library.artistPage(id).catch(() => null)
  if (page.value) setPageTitle(here, page.value.artist.name)
}
load()
const stop = library.onChange((change) => change.sources.includes(source) && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const artist = computed(() => page.value?.artist)
const songs = computed(() => page.value?.tracks.map(trackSong) ?? [])
const featured = computed(() => page.value?.featured.map(trackSong) ?? [])
const line = computed(() => {
  const a = artist.value
  if (!a) return ''
  return ['Artist', count(a.tracks + a.featured, 'song'), ...(a.albums ? [count(a.albums, 'album')] : [])].join(' · ')
})
const ALBUM_MENU = coverMenu({ kind: 'album' })

</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <template v-if="artist">
      <PageHeader :title="artist.name" :subtitle="line">
        <template #actions>
          <Button size="lg" @click="playList(source)"><PlayIcon class="size-3.5" />Play</Button>
          <Button size="lg" variant="outline" @click="playList(source, { shuffle: true })"><Shuffle />Shuffle</Button>
        </template>
      </PageHeader>

      <section v-if="page?.albums.length" class="mb-10">
        <h2 class="mb-2 text-xl font-semibold">Albums</h2>
        <div class="cover-grid [--cover-min:10.5rem]">
          <CoverCard
            v-for="album in page.albums"
            :key="album.id"
            :title="album.title"
            :subtitle="[album.artist, album.year].filter(Boolean).join(' · ')"
            :image="album.cover ? coverUrl(album.cover) : undefined"
            :menu="ALBUM_MENU"
            @open="router.push(`/album/${album.id}`)"
            @action="(action) => listAction(`album:${album.id}`, action as CoverAction, album.artistId)"
          />
        </div>
      </section>

      <section v-if="songs.length" class="mb-10">
        <h2 class="mb-2 text-xl font-semibold">Songs</h2>
        <SongList
          :songs="songs"
          :current-id="current?.id"
          :playing="!!nowPlaying.state?.isPlaying"
          :place="{ showInFolder: isDesktop }"
          @play="playSong"
          @pause="togglePlay"
          @like="(song, liked) => like(Number(song.id), liked)"
          @action="songAction"
        />
      </section>

      <section v-if="featured.length" class="mb-10">
        <h2 class="mb-2 text-xl font-semibold">Featured on</h2>
        <SongList
          :songs="featured"
          :current-id="current?.id"
          :playing="!!nowPlaying.state?.isPlaying"
          :place="{ showInFolder: isDesktop }"
          @play="playSong"
          @pause="togglePlay"
          @like="(song, liked) => like(Number(song.id), liked)"
          @action="songAction"
        />
      </section>
    </template>
    <EmptyState v-else-if="page === null" :icon="User" title="This artist isn’t in your library" text="Their songs may have been removed, or their folder switched off.">
      <Button size="sm" variant="outline" @click="goBack">Back</Button>
    </EmptyState>
  </div>
</template>
