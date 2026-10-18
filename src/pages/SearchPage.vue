<script setup lang="ts">
/**
 * Search (opened from the top bar's field, Ctrl Space or "/"): its own big field, focused, and what's typed kept in the
 * address (?q=), so going back to it shows the same search. Escape empties the field, then closes Search.
 *
 * Before typing: Recently added songs. Typing (a moment after the last key): Songs, Artists, Albums, Playlists and
 * Settings whose names have it anywhere (accents and case ignored); an artist's songs and albums come with them. Each
 * group shows its first few, See all the rest. A setting's link pasted here, Enter: Settings, at that setting.
 */
import { Search, X } from '@lucide/vue'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { CoverCard, coverMenu, type CoverAction } from '@/components/covers'
import { LinkRow, PageHeader } from '@/components/page'
import { SongList, songMenu, type Song, type SongAction } from '@/components/songs'
import { coverUrl, library, type Track } from '@/library'
import { count } from '@/lib/format'
import { isDesktop } from '@/platform/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { listAction, playSongs, songAction, trackSong } from '@/player/songs'
import { goBack } from '@/router'
import { SETTINGS } from '@/settings/registry'
import { LINK } from '@/settings/settingsMenu'
import { playlistCover, playlistLine } from './playlists/playlists'

const route = useRoute()
const router = useRouter()
const field = ref<HTMLInputElement>()
const q = ref(typeof route.query.q === 'string' ? route.query.q : '')

// Focused once it has slid in (focusing while it moves would scroll it).
onMounted(() => field.value?.focus({ preventScroll: true }))
const typed = () => router.replace({ query: q.value ? { q: q.value } : {} })
function enter() {
  const text = q.value.trim()
  if (text.toLowerCase().startsWith(LINK)) router.push({ path: '/settings', query: { setting: text.slice(LINK.length) } })
}
function escape() {
  if (q.value) {
    q.value = ''
    typed()
  } else goBack()
}

// ---- Before typing: recently added songs
const added = ref<Track[]>([])
library.tracks(0, 14, 'added').then((t) => (added.value = t)).catch(() => {})
const addedSongs = computed(() => added.value.map(trackSong))
// A song card's menu: the song's own (Add to playlist is in a song row's menu).
const cardMenu = (song: Song) => songMenu(song).filter((i) => i.action !== 'addToPlaylist')

// ---- Results, a moment after the last key
const SONGS = 200
type Found = Awaited<ReturnType<typeof library.search>>
const found = ref<Found | null>(null)
// What the results shown were found for: new ones cross-fade in (the old stay until then, so nothing jumps).
const foundFor = ref('')
// Looking: a skeleton while a search's first results come (not over results already shown, on every key).
const looking = ref(false)
const query = computed(() => q.value.trim())
let waiting: ReturnType<typeof setTimeout> | undefined
watch(
  query,
  (text) => {
    clearTimeout(waiting)
    if (!text || text.toLowerCase().startsWith(LINK)) {
      looking.value = false
      found.value = null
      return
    }
    looking.value = true
    waiting = setTimeout(async () => {
      const result = await library.search(text, SONGS).catch(() => null)
      if (query.value !== text) return
      found.value = result
      foundFor.value = text
      looking.value = false
    }, 150)
  },
  { immediate: true },
)
const stop = library.onChange(() => query.value && library.search(query.value, SONGS).then((r) => (found.value = r)).catch(() => {}))
onUnmounted(() => stop.then((unlisten) => unlisten()))

// Settings whose name (or what they do) has it.
const plain = (text: string) => text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()
const settings = computed(() => {
  const text = plain(foundFor.value)
  if (!text || text.startsWith(LINK)) return []
  const all = Object.entries(SETTINGS)
  return [...all.filter(([, s]) => plain(s.title).includes(text)), ...all.filter(([, s]) => !plain(s.title).includes(text) && plain(s.help).includes(text))]
})

// Each group: its first few, See all for the rest.
const FEW = { songs: 5, artists: 5, albums: 6, playlists: 6, settings: 5 }
const all = ref<Record<keyof typeof FEW, boolean>>({ songs: false, artists: false, albums: false, playlists: false, settings: false })
watch(query, () => (all.value = { songs: false, artists: false, albums: false, playlists: false, settings: false }))
const few = <T,>(list: T[], group: keyof typeof FEW) => (all.value[group] ? list : list.slice(0, FEW[group]))
const songs = computed(() => (found.value?.tracks ?? []).map(trackSong))
const nothing = computed(
  () => found.value && !found.value.trackCount && !found.value.albums.length && !found.value.artists.length && !found.value.playlists.length && !settings.value.length,
)

function playFound(song: Song) {
  if (song.id === current.value?.id) togglePlay()
  else playSongs((found.value?.tracks ?? []).map((t) => t.id), Number(song.id))
}
const ALBUM_MENU = coverMenu({ kind: 'album' })
const LIST_MENU = coverMenu({ kind: 'list' })
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader title="Search" subtitle="Find anything in the library" />
    <label class="mb-8 flex h-[52px] items-center gap-3 rounded-xl border border-border bg-card px-4 dark:border-transparent">
      <Search class="size-5 flex-none text-muted-foreground" />
      <input
        ref="field"
        v-model="q"
        type="search"
        placeholder="Songs, albums, artists, playlists, settings"
        class="min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-muted-foreground"
        @input="typed"
        @keydown.esc.prevent="escape"
        @keydown.enter.prevent="enter"
      />
      <button
        v-if="q"
        type="button"
        class="grid size-8 flex-none place-items-center rounded-md text-muted-foreground transition-colors hover:bg-well hover:text-foreground"
        aria-label="Clear"
        @click="(q = ''), typed(), field?.focus()"
      >
        <X class="size-4" />
      </button>
    </label>

    <!-- Recently added (before typing), the skeleton (a search's first results coming), the results: each change
         fades, the new one rising a little. -->
    <Transition name="part" mode="out-in">
      <section v-if="!query" key="recent">
        <h2 class="mb-2 text-xl font-semibold">Recently added songs</h2>
        <div class="cover-grid [--cover-min:10.5rem]">
          <CoverCard
            v-for="song in addedSongs"
            :key="song.id"
            :title="song.title"
            :subtitle="song.artist"
            :image="song.cover"
            :menu="cardMenu(song)"
            @open="playSongs(added.map((t) => t.id), Number(song.id))"
            @action="(action) => songAction(song, action as SongAction)"
          />
        </div>
      </section>

      <!-- The first results of a search coming: a skeleton (a group's heading and its rows). It pulses inside: an endless
           animation on the fading element itself would keep its fade from ending. -->
      <div v-else-if="looking && !found" key="looking" aria-hidden="true">
        <div class="animate-pulse">
          <div class="mb-4 h-5 w-24 rounded bg-card" />
          <div v-for="row in 5" :key="row" class="flex h-[68px] items-center gap-4 px-4">
            <div class="size-12 flex-none rounded-md bg-card" />
            <div class="flex flex-1 flex-col gap-2">
              <div class="h-3.5 w-1/3 rounded bg-card" />
              <div class="h-3 w-1/5 rounded bg-card" />
            </div>
          </div>
        </div>
      </div>

      <div v-else-if="found" :key="`found:${foundFor}`">
        <p v-if="nothing" class="text-lg text-muted-foreground">Nothing found for “{{ foundFor }}”.</p>

        <section v-if="songs.length" class="mb-10">
          <div class="mb-2 flex items-baseline justify-between">
            <h2 class="text-xl font-semibold">Songs</h2>
            <button v-if="found.trackCount > FEW.songs" type="button" class="text-sm text-muted-foreground hover:text-foreground hover:underline" @click="all.songs = !all.songs">
              {{ all.songs ? 'Show less' : `See all ${found.trackCount.toLocaleString()}` }}
            </button>
          </div>
          <SongList
            :songs="few(songs, 'songs')"
            :current-id="current?.id"
            :playing="!!nowPlaying.state?.isPlaying"
            :place="{ showInFolder: isDesktop }"
            @play="playFound"
            @pause="togglePlay"
            @like="(song, liked) => like(Number(song.id), liked)"
            @action="songAction"
          />
          <p v-if="all.songs && found.trackCount > SONGS" class="mt-2 text-sm text-muted-foreground">The first {{ SONGS }}: type more to narrow it down.</p>
        </section>

        <section v-if="found.artists.length" class="mb-10">
          <div class="mb-2 flex items-baseline justify-between">
            <h2 class="text-xl font-semibold">Artists</h2>
            <button v-if="found.artists.length > FEW.artists" type="button" class="text-sm text-muted-foreground hover:text-foreground hover:underline" @click="all.artists = !all.artists">
              {{ all.artists ? 'Show less' : `See all ${found.artists.length}` }}
            </button>
          </div>
          <LinkRow
            v-for="artist in few(found.artists, 'artists')"
            :key="artist.id"
            :to="`/artist/${artist.id}`"
            :title="artist.name"
            :subtitle="count(artist.tracks + artist.featured, 'song')"
            :menu="LIST_MENU"
            @action="(action) => listAction(`artist:${artist.id}`, action as CoverAction)"
          />
        </section>

        <section v-if="found.albums.length" class="mb-10">
          <div class="mb-2 flex items-baseline justify-between">
            <h2 class="text-xl font-semibold">Albums</h2>
            <button v-if="found.albums.length > FEW.albums" type="button" class="text-sm text-muted-foreground hover:text-foreground hover:underline" @click="all.albums = !all.albums">
              {{ all.albums ? 'Show less' : `See all ${found.albums.length}` }}
            </button>
          </div>
          <div class="cover-grid [--cover-min:10.5rem]">
            <CoverCard
              v-for="album in few(found.albums, 'albums')"
              :key="album.id"
              :title="album.title"
              :subtitle="album.artist ?? 'Various artists'"
              :image="album.cover ? coverUrl(album.cover) : undefined"
              :menu="ALBUM_MENU"
              @open="router.push(`/album/${album.id}`)"
              @action="(action) => listAction(`album:${album.id}`, action as CoverAction, album.artistId)"
            />
          </div>
        </section>

        <section v-if="found.playlists.length" class="mb-10">
          <div class="mb-2 flex items-baseline justify-between">
            <h2 class="text-xl font-semibold">Playlists</h2>
            <button v-if="found.playlists.length > FEW.playlists" type="button" class="text-sm text-muted-foreground hover:text-foreground hover:underline" @click="all.playlists = !all.playlists">
              {{ all.playlists ? 'Show less' : `See all ${found.playlists.length}` }}
            </button>
          </div>
          <div class="cover-grid [--cover-min:10.5rem]">
            <CoverCard
              v-for="p in few(found.playlists, 'playlists')"
              :key="p.id"
              :title="p.name"
              :subtitle="playlistLine(p)"
              v-bind="playlistCover(p)"
              :menu="LIST_MENU"
              @open="router.push(`/playlist/${p.id}`)"
              @action="(action) => listAction(`playlist:${p.id}`, action as CoverAction)"
            />
          </div>
        </section>

        <section v-if="settings.length" class="mb-10">
          <div class="mb-2 flex items-baseline justify-between">
            <h2 class="text-xl font-semibold">Settings</h2>
            <button v-if="settings.length > FEW.settings" type="button" class="text-sm text-muted-foreground hover:text-foreground hover:underline" @click="all.settings = !all.settings">
              {{ all.settings ? 'Show less' : `See all ${settings.length}` }}
            </button>
          </div>
          <LinkRow
            v-for="[id, setting] in few(settings, 'settings')"
            :key="id"
            :to="{ path: '/settings', query: { setting: id } }"
            :title="setting.title"
            :subtitle="setting.help"
          />
        </section>
      </div>
    </Transition>
  </div>
</template>
