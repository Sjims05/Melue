<script setup lang="ts">
/**
 * Home (Melue's design): the time of day; the Playing card (the song playing, opening Now playing) and, when another
 * device was playing, "Continue from <device>?" (Not now / Continue), both in the primary colour or like the cards
 * (Display settings); then Recently played (the lists you played, newest first: one row, See all for the rest) and
 * Recent songs (the last ones you played).
 */
import { CircleAlert, History, Music } from '@lucide/vue'
import { computed, onActivated, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Cover, CoverCard, coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, PageHeader } from '@/components/page'
import { SongList, type Song } from '@/components/songs'
import { Button } from '@/components/ui/button'
import { library, type Track } from '@/library'
import { RECENT_LISTS } from '@/library/playback'
import { minutesAndSeconds, playbackSync, sync, type PlaybackFile } from '@/library/sync'
import { appearance, primaryArea } from '@/lib/appearance'
import { isDesktop, trackedLists } from '@/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { listAction, playSong, songAction, trackSong } from '@/player/songs'
import { rememberedHome, rememberHome } from './home/homeMemory'
import { listCards, type ListCard } from './home/listCards'

const router = useRouter()

// The grey line: the time of day, as in the design ("Late night").
const part = computed(() => {
  const h = new Date().getHours()
  return h < 5 ? 'Late night' : h < 12 ? 'Morning' : h < 17 ? 'Afternoon' : h < 22 ? 'Evening' : 'Late night'
})
const cardClass = computed(() => (appearance.homeCardsPrimary ? primaryArea.value : 'bg-card text-card-foreground'))

// ---- Another device's playback, offered
const offer = ref<PlaybackFile | null>(null)
const stops = [playbackSync.onOffer((o) => (offer.value = o))]
async function answer(go: boolean) {
  await (go ? playbackSync.continue() : playbackSync.notNow()).catch(() => {})
  offer.value = null
}

// ---- Recently played lists, and recent songs (read again whenever Home is shown)
const ROW = 6
// Until the library answers, Home as it was last shown (homeMemory.ts): there at once at start.
const remembered = rememberedHome()
const lists = ref<ListCard[]>(remembered?.lists ?? [])
const allLists = ref(false)
const recent = ref<Track[]>(remembered?.recent ?? [])
// No songs at all (a fresh start): Home welcomes you instead. The library can't be read: it says so.
const noMusic = ref(false)
const problem = ref('')
async function load() {
  problem.value = ''
  const [played, songs, offered, status] = await Promise.all([
    trackedLists.get(RECENT_LISTS).catch(() => []),
    library.recentlyPlayed(8).catch(() => []),
    playbackSync.offer().catch(() => null),
    library.status().catch((e) => void (problem.value = String(e))),
  ])
  noMusic.value = !!status && status.tracks === 0
  lists.value = await listCards(played.map((e) => String(e.id)))
  recent.value = songs
  offer.value = offered
  if (!problem.value) rememberHome(lists.value, songs)
}
onActivated(load)
stops.push(library.onChange((change) => change.reason !== 'likes' && load()))
onUnmounted(() => stops.forEach((stop) => stop.then((unlisten) => unlisten())))

const shownLists = computed(() => (allLists.value ? lists.value : lists.value.slice(0, ROW)))
const songs = computed(() => recent.value.map(trackSong))
const MENU = coverMenu({ kind: 'list' })
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader title="Home" :subtitle="part" />

    <!-- What's playing here, and what another device was playing. The space opens and closes as they come and go; a new
         song's cover fades in over the old one, its words cross-fade. -->
    <div
      class="grid transition-[grid-template-rows,opacity] duration-300 ease-[cubic-bezier(.2,.8,.2,1)]"
      :class="current || offer ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'"
    >
      <div class="min-h-0 overflow-hidden">
        <div class="grid gap-4 pb-10 lg:grid-cols-2">
          <Transition name="fade">
            <RouterLink
              v-if="current"
              to="/now-playing"
              class="flex min-w-0 items-center gap-4 rounded-xl p-4 transition-opacity outline-none hover:opacity-90 focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-inset"
              :class="cardClass"
            >
              <span class="w-16 flex-none [&>div]:rounded-md"><Cover :image="current.cover" /></span>
              <span class="flex min-w-0 flex-col">
                <Transition name="swap" mode="out-in">
                  <span :key="nowPlaying.state?.isPlaying ? 'playing' : 'paused'" class="text-sm text-muted-foreground">{{
                    nowPlaying.state?.isPlaying ? 'Playing' : 'Paused'
                  }}</span>
                </Transition>
                <Transition name="swap" mode="out-in">
                  <span :key="current.id" class="flex min-w-0 flex-col">
                    <span class="truncate text-[17px] font-semibold">{{ current.title }}</span>
                    <span class="truncate text-muted-foreground">{{ current.artist }}</span>
                  </span>
                </Transition>
              </span>
            </RouterLink>
          </Transition>
          <!-- On a phone its buttons go on a line of their own, at the right. It fades away once answered. -->
          <Transition name="fade">
            <div v-if="offer" class="flex min-w-0 flex-wrap items-center gap-4 rounded-xl p-4 phone:gap-x-4 phone:gap-y-3" :class="cardClass">
              <span class="grid size-16 flex-none place-items-center rounded-md bg-well text-muted-foreground"><Music class="size-6" /></span>
              <span class="flex min-w-0 flex-1 flex-col phone:basis-[calc(100%-5rem)]">
                <span class="truncate text-sm text-muted-foreground">Continue from {{ offer.deviceName }}?</span>
                <span class="truncate text-[17px] font-semibold">{{ offer.title ?? 'A song' }}</span>
                <span class="truncate text-muted-foreground">{{ [offer.artist, minutesAndSeconds(offer.positionS)].filter(Boolean).join(' · ') }}</span>
              </span>
              <Button variant="outline" size="sm" class="phone:ml-auto" @click="answer(false)">Not now</Button>
              <Button size="sm" @click="answer(true)">Continue</Button>
            </div>
          </Transition>
        </div>
      </div>
    </div>

    <!-- A problem, the welcome (no music yet), or Home itself: a fade between them. -->
    <Transition name="part" mode="out-in">
      <EmptyState v-if="problem" :icon="CircleAlert" title="Can’t read your library" :text="problem" problem>
        <Button size="sm" variant="outline" @click="load">Try again</Button>
      </EmptyState>
      <EmptyState
        v-else-if="noMusic"
        :icon="Music"
        title="Add your music"
        text="Melue plays the music in your folders. With a share it keeps your music, likes and playlists the same on all your devices."
      >
        <Button size="sm" @click="router.push('/settings')">Add a music folder</Button>
        <Button v-if="!sync.available" size="sm" variant="outline" @click="router.push('/settings')">Set up a share</Button>
      </EmptyState>
      <div v-else class="grid gap-12 lg:grid-cols-2">
        <section>
          <div class="mb-2 flex items-center justify-between">
            <h2 class="text-xl font-semibold">Recently played</h2>
            <button
              v-if="lists.length > ROW"
              type="button"
              class="text-sm text-muted-foreground hover:text-foreground hover:underline"
              @click="allLists = !allLists"
            >
              {{ allLists ? 'Show less' : 'See all' }}
            </button>
          </div>
          <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
          <Transition name="part" mode="out-in">
          <div v-if="lists.length" class="cover-grid [--cover-min:10rem]">
            <CoverCard
              v-for="list in shownLists"
              :key="list.source"
              :title="list.title"
              :subtitle="list.subtitle"
              :image="list.image"
              :collage="list.collage"
              :colors="list.colors"
              :icon="list.icon"
              :placeholder="list.placeholder"
              :menu="MENU"
              @open="router.push(list.page)"
              @action="(action) => listAction(list.source, action as CoverAction)"
            />
          </div>
          <EmptyState v-else :icon="History" title="Nothing played yet" text="Albums, playlists and artists you play show up here." small />
          </Transition>
        </section>
        <section>
          <h2 class="mb-2 text-xl font-semibold">Recent songs</h2>
          <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
          <Transition name="part" mode="out-in">
          <SongList
            v-if="songs.length"
            :songs="songs"
            :current-id="current?.id"
            :playing="!!nowPlaying.state?.isPlaying"
            :place="{ showInFolder: isDesktop }"
            @play="playSong"
            @pause="togglePlay"
            @like="(song, liked) => like(Number(song.id), liked)"
            @action="songAction"
          />
          <EmptyState v-else :icon="Music" title="No songs played yet" text="Songs you play show up here." small />
          </Transition>
        </section>
      </div>
    </Transition>
  </div>
</template>
