<script setup lang="ts">
/**
 * Library's Songs: every song, in the chosen order, A–Z headings when sorted by title, artist or album. Only the rows on
 * screen are drawn (VirtualSongList), so a big library scrolls smoothly. A song's play button plays just it (now or
 * next: Queue settings), a double-click adds it to Up next; Play and Shuffle above play them all. It follows the
 * library: a scan or a folder change shows at once.
 */
import { CircleAlert, Music } from '@lucide/vue'
import { computed, onUnmounted, ref, watch } from 'vue'
import { letterOf, VirtualSongList, type Song } from '@/components/songs'
import { library, type Track, type TrackSort } from '@/library'
import { isDesktop } from '@/platform/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { playSong, songAction, trackSong } from '@/player/songs'
import { useRouter } from 'vue-router'
import { EmptyState } from '@/components/page'
import { Button } from '@/components/ui/button'

const props = defineProps<{ sort: TrackSort }>()

// All of them at once (a few thousand are quick to read); the list draws only what's on screen.
const tracks = ref<Track[] | null>(null)
// Why the list couldn't be read (shown instead of an empty list).
const problem = ref('')
const router = useRouter()
async function load() {
  problem.value = ''
  tracks.value = await library.tracks(0, 1_000_000, props.sort).catch((e) => ((problem.value = String(e)), []))
}
watch(() => props.sort, load, { immediate: true })
// Likes show through the player's liked songs (at once, everywhere); anything else reloads the list.
const stop = library.onChange((change) => change.reason !== 'likes' && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const songs = computed(() => (tracks.value ?? []).map(trackSong))
const headingOf = computed<((song: Song) => string) | undefined>(() => {
  if (props.sort === 'title') return (s) => letterOf(s.title)
  if (props.sort === 'artist') return (s) => letterOf(s.artists?.[0]?.name ?? s.artist)
  if (props.sort === 'album') return (s) => (s.album ? letterOf(s.album) : 'Not on an album')
  return undefined
})

// Sorted by plays or date added: each song's count or date where the queue has its numbers.
const DAY = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short' })
const MONTH = new Intl.DateTimeFormat(undefined, { month: 'short', year: 'numeric' })
const LONG = new Intl.DateTimeFormat(undefined, { dateStyle: 'long' })
const numberOf = computed<((song: Song) => { text: string; title: string }) | undefined>(() => {
  if (props.sort === 'plays') {
    return (s) => ({ text: String(s.plays ?? 0), title: `${s.plays ?? 0} play${s.plays === 1 ? '' : 's'}` })
  }
  if (props.sort === 'added') {
    const thisYear = new Date().getFullYear()
    return (s) => {
      const added = new Date(s.addedAt ?? 0)
      return { text: (added.getFullYear() === thisYear ? DAY : MONTH).format(added), title: `Added ${LONG.format(added)}` }
    }
  }
  return undefined
})

const place = { showInFolder: isDesktop }
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
    <Transition name="part" mode="out-in">
      <VirtualSongList
        v-if="tracks?.length"
        :songs="songs"
        :heading-of="headingOf"
        :number-of="numberOf"
        :current-id="current?.id"
        :playing="!!nowPlaying.state?.isPlaying"
        :place="place"
        @play="playSong"
        @pause="togglePlay"
        @like="(song, liked) => like(Number(song.id), liked)"
        @action="songAction"
      />
      <!-- Can't be read (Try again), or nothing yet (where to add music). -->
      <EmptyState v-else-if="problem" :icon="CircleAlert" title="Can’t read your library" :text="problem" problem>
        <Button size="sm" variant="outline" @click="load">Try again</Button>
      </EmptyState>
      <EmptyState v-else-if="tracks" :icon="Music" title="No songs yet" text="Add a music folder, and its songs show up here.">
        <Button size="sm" @click="router.push('/settings')">Add a music folder</Button>
      </EmptyState>
    </Transition>
  </div>
</template>
