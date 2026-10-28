<script setup lang="ts">
/**
 * A Made for you list (opened on top, like your playlists): All songs (A–Z), Liked songs (newest liked first), or a
 * Top songs list (most played first, numbered, with how many it holds: 25, 50, 100 or 200, chosen here for every Top songs list). Its cover,
 * "Made for you", its name and line, Play, Shuffle and ⋯ (its card's menu: Hide). The songs scroll like the Songs tab
 * (only what's on screen is drawn). It follows the library and your likes.
 */
import { ListMusic, Sparkles } from '@lucide/vue'
import { computed, ref } from 'vue'
import { useRoute, type RouteLocationNormalized } from 'vue-router'
import { coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, ListHeader } from '@/components/page'
import { Segmented, SettingsBadge } from '@/components/settings'
import { letterOf, VirtualSongList, type Song } from '@/components/songs'
import { AUTO_PLAYLIST_SIZES, library, type AutoPlaylist, type Source, type Track } from '@/library'
import { count, totalLength } from '@/lib/format'
import { isDesktop } from '@/platform/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { exportList, listAction, playList, playSong, songAction, trackSong } from '@/player/songs'
import { goBack, setPageTitle } from '@/router'
import { settingsTouched } from '@/settings/sharedSettings'
import { autoCard } from './playlists/playlists'
import { Button } from '@/components/ui/button'
import { onLibraryChange } from '@/library/changes'

const route = useRoute()
// This page's own address and list (the route moves on when another page opens on top of it).
const here = { fullPath: route.fullPath } as RouteLocationNormalized
// /playlist/songs, /playlist/liked, /playlist/top-month: the list's source ('top:month').
const source = String(route.params.auto).replace(/^top-/, 'top:') as Source

// undefined while it loads; null: no such list (or hidden).
const auto = ref<AutoPlaylist | null>()
const tracks = ref<Track[]>([])
async function load() {
  const [autos, found] = await Promise.all([library.autoPlaylists().catch(() => []), library.autoPlaylistTracks(source).catch(() => [])])
  auto.value = autos.find((a) => a.source === source) ?? null
  tracks.value = found
  if (auto.value) setPageTitle(here, auto.value.name)
}
load()
// Liked songs changes with every like; the others with the library.
onLibraryChange((change) => change.reason !== 'likes' || source === 'liked', load)

const card = computed(() => (auto.value ? autoCard(auto.value) : null))
// "Last month · 50 songs · 3 h 10 min" (a Top songs list's period first), "Playlist · 86 songs · 5 h 2 min".
const line = computed(() => {
  const period = auto.value?.name.split(': ')[1]
  const what = period ? period.charAt(0).toUpperCase() + period.slice(1) : 'Playlist'
  return [what, count(songs.value.length, 'song'), ...(songs.value.length ? [totalLength(length.value)] : [])].join(' · ')
})
const songs = computed(() => tracks.value.map(trackSong))
const length = computed(() => tracks.value.reduce((sum, t) => sum + (t.durationMs ?? 0), 0))
const top = source.startsWith('top:')
const headingOf = source === 'songs' ? (s: Song) => letterOf(s.title) : undefined
// Top songs: its place, and the plays under the pointer.
const rank = computed(() => new Map(songs.value.map((s, i) => [s.id, i + 1])))
const numberOf = top ? (s: Song) => ({ text: String(rank.value.get(s.id) ?? ''), title: count(s.plays ?? 0, 'play') }) : undefined
const SIZES = AUTO_PLAYLIST_SIZES.map((n) => [n, String(n)] as const)
// One size for all the Top songs lists (as Settings has it): changed here, changed for each.
async function resize(size: number) {
  const tops = (await library.autoPlaylists()).filter((p) => p.kind === 'top')
  await Promise.all(tops.map((p) => library.setAutoPlaylist(p.source, { size })))
  settingsTouched()
  await load()
}

const MENU = coverMenu({ kind: 'auto' }, isDesktop)
async function pick(action: CoverAction) {
  if (action === 'export') return exportList(source, auto.value?.name.replace(': ', ', ') ?? 'Playlist')
  await listAction(source, action)
  if (action === 'hide') {
    await library.setAutoPlaylist(source, { hidden: true })
    goBack()
  }
}
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <template v-if="auto && card">
      <ListHeader
        :title="card.title"
        :colors="card.colors"
        :icon="card.icon"
        :empty="!songs.length"
        :menu="MENU"
        @action="(action) => pick(action as CoverAction)"
        @play="playList(source)"
        @shuffle="playList(source, { shuffle: true })"
      >
        <template #badges>
          <SettingsBadge class="h-7 gap-1.5 px-2.5 text-[13px]"><Sparkles class="size-3.5" />Made for you</SettingsBadge>
        </template>
        <template #line>{{ line }}</template>
        <template #actions>
          <!-- On a phone: a row of its own, under Play and Shuffle. -->
          <div v-if="top && auto.size" class="flex items-center phone:order-last phone:w-full">
            <Segmented :model-value="auto.size" :options="SIZES" @update:model-value="resize" />
          </div>
        </template>
      </ListHeader>
      <VirtualSongList
        v-if="songs.length"
        :songs="songs"
        :heading-of="headingOf"
        :number-of="numberOf"
        :current-id="current?.id"
        :playing="!!nowPlaying.state?.isPlaying"
        :place="{ showInFolder: isDesktop }"
        @play="playSong"
        @pause="togglePlay"
        @like="(song, liked) => like(Number(song.id), liked)"
        @action="songAction"
      />
      <p v-else class="text-muted-foreground">
        {{ source === 'liked' ? 'No liked songs yet: the heart on a song likes it.' : top ? 'Nothing played in this time yet.' : 'No songs yet.' }}
      </p>
    </template>
    <EmptyState v-else-if="auto === null" :icon="ListMusic" title="This list isn’t in your library" text="It may have been hidden: Settings has it under Hidden auto playlists.">
      <Button size="sm" variant="outline" @click="goBack">Back</Button>
    </EmptyState>
  </div>
</template>
