<script setup lang="ts">
/**
 * Library: what the songs bring (Songs A–Z, Albums, Artists, Folders), as tabs. Playlists are their own page (made by
 * you, or by Melue for you). The tab is kept in the address (?tab=albums), so going back returns to it. Songs, Albums
 * and Artists each have a sort menu at the tabs' right; each remembers its order.
 */
import { useLocalStorage } from '@vueuse/core'
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { PageHeader, PlannedCard } from '@/components/page'
import { Segmented } from '@/components/settings'
import { library, type AlbumSort, type ArtistSort, type LibraryStatus, type TrackSort } from '@/library'
import AlbumsTab from './library/AlbumsTab.vue'
import ArtistsTab from './library/ArtistsTab.vue'
import SongsTab from './library/SongsTab.vue'
import SortMenu from './library/SortMenu.vue'

type Tab = 'songs' | 'albums' | 'artists' | 'folders'
const TABS = [['songs', 'Songs'], ['albums', 'Albums'], ['artists', 'Artists'], ['folders', 'Folders']] as const

const route = useRoute()
const router = useRouter()
// The tab is the page's own (it's kept alive under a page opened on top, whose address isn't Library's); it follows
// the address while Library is shown (a link to ?tab=albums), and changing it updates the address.
const fromAddress = () => (TABS.some(([t]) => t === route.query.tab) ? (route.query.tab as Tab) : 'songs')
const current = ref<Tab>(fromAddress())
watch(() => route.fullPath, () => route.name === 'library' && (current.value = fromAddress()))
const tab = computed<Tab>({
  get: () => current.value,
  set: (t) => {
    current.value = t
    router.replace({ query: t === 'songs' ? {} : { tab: t } })
  },
})

// Each tab's order, remembered on this device.
const SONG_SORTS = [['title', 'Title'], ['artist', 'Artist'], ['album', 'Album'], ['added', 'Date added'], ['plays', 'Most played']] as const
const ALBUM_SORTS = [['title', 'Title'], ['artist', 'Artist'], ['year', 'Year'], ['added', 'Date added']] as const
const ARTIST_SORTS = [['name', 'Name'], ['songs', 'Most songs']] as const
const songSort = useLocalStorage<TrackSort>('melue.library.songSort', 'title')
const albumSort = useLocalStorage<AlbumSort>('melue.library.albumSort', 'title')
const artistSort = useLocalStorage<ArtistSort>('melue.library.artistSort', 'name')

const status = ref<LibraryStatus | null>(null)
onMounted(async () => (status.value = await library.status().catch(() => null)))
const counts = computed(() => {
  const s = status.value
  if (!s) return ''
  const n = (k: number, one: string) => `${k.toLocaleString()} ${one}${k === 1 ? '' : 's'}`
  return [n(s.tracks, 'song'), n(s.albums, 'album'), n(s.artists, 'artist')].join(' · ')
})
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader title="Library" :subtitle="counts">
      <div class="flex items-center justify-between gap-4">
        <Segmented v-model="tab" :options="TABS" />
        <SortMenu v-if="tab === 'songs'" v-model="songSort" :options="SONG_SORTS" />
        <SortMenu v-else-if="tab === 'albums'" v-model="albumSort" :options="ALBUM_SORTS" />
        <SortMenu v-else-if="tab === 'artists'" v-model="artistSort" :options="ARTIST_SORTS" />
      </div>
    </PageHeader>
    <SongsTab v-if="tab === 'songs'" :sort="songSort" />
    <AlbumsTab v-else-if="tab === 'albums'" :sort="albumSort" />
    <ArtistsTab v-else-if="tab === 'artists'" :sort="artistSort" />
    <PlannedCard v-else :title="TABS.find(([t]) => t === tab)![1]" text="Your music folders, with their own pages: the coming step (Library)." />
  </div>
</template>
