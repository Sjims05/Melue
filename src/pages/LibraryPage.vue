<script setup lang="ts">
/**
 * Library: what the songs bring (Songs A–Z, Albums, Artists, Folders), as tabs. Playlists are their own page (made by
 * you, or by Melue for you). The tab is kept in the address (?tab=albums), so going back returns to it.
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { PageHeader, PlannedCard } from '@/components/page'
import { Segmented } from '@/components/settings'
import { library, type LibraryStatus } from '@/library'

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
      <Segmented v-model="tab" :options="TABS" />
    </PageHeader>
    <PlannedCard :title="TABS.find(([t]) => t === tab)![1]" text="The songs A–Z, albums, artists and folders, with their own pages: a coming step (Library)." />
  </div>
</template>
