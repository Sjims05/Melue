<script setup lang="ts">
/**
 * Playlists (Melue's design): "6 playlists · 8 made for you". Your playlists: cards (their covers' collage, the
 * Synced / This device / Linked chip, "12 songs · 48 min"); + makes a new one, its name ready to type. Right-click: play
 * it, rename it (its name turns into a text field), keep it only here or sync it, stop following its file, delete it
 * (both asked first), and About how it lives (actions.ts, PlaylistDialogs).
 * A synced playlist changed on two devices that don't agree says it needs a choice. Then Made for you: All songs,
 * Liked songs, the Top songs lists and the music folders (with two or more), each can be hidden (Settings shows them
 * again). It follows the library and the sync.
 */
import { AudioLines, CircleAlert, ListMusic, Plus } from '@lucide/vue'
import { computed, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { CoverBadge, CoverCard, coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, PageHeader } from '@/components/page'
import { SettingsTip } from '@/components/settings'
import { Button } from '@/components/ui/button'
import { library, type AutoPlaylist, type DeletedLinkedPlaylist, type Playlist } from '@/library'
import { librarySync, sync, type DeletedPlaylist, type PlaylistConflict } from '@/library/sync'
import { count } from '@/lib/format'
import { isDesktop } from '@/platform/platform'
import { exportList, listAction } from '@/player/songs'
import { usePlaylistActions } from './playlists/actions'
import ConflictDialog from './playlists/ConflictDialog.vue'
import CoverCropDialog from './playlists/CoverCropDialog.vue'
import PlaylistDialogs from './playlists/PlaylistDialogs.vue'
import SpotifyUpdateDialog from './playlists/SpotifyUpdateDialog.vue'
import SpotifyAddDialog from './playlists/SpotifyAddDialog.vue'
import { autoCard, autoPage, playlistBadge, playlistCover, playlistKind, playlistLine } from './playlists/playlists'
import { onLibraryChange } from '@/library/changes'

const router = useRouter()

const playlists = ref<Playlist[]>([])
const autos = ref<AutoPlaylist[]>([])
const deleted = ref<(DeletedPlaylist | DeletedLinkedPlaylist)[]>([])
const conflicts = ref<PlaylistConflict[]>([])
// Why the playlists couldn't be read (shown instead of "No playlists yet").
const problem = ref('')
async function load() {
  problem.value = ''
  ;[playlists.value, autos.value, deleted.value, conflicts.value] = await Promise.all([
    library.playlists().catch((e) => ((problem.value = String(e)), [])),
    library.autoPlaylists().catch(() => []),
    Promise.all([librarySync.deletedPlaylists().catch(() => []), library.deletedLinkedPlaylists().catch(() => [])]).then((lists) => lists.flat()),
    librarySync.conflicts().catch(() => []),
  ])
}
load()
const stops = [
  librarySync.onConflicts((list) => (conflicts.value = list)),
]
onUnmounted(() => stops.forEach((stop) => stop.then((unlisten) => unlisten())))
onLibraryChange((change) => change.reason !== 'likes', load)

const shown = computed(() => autos.value.filter((a) => !a.hidden))
const subtitle = computed(() => [count(playlists.value.length, 'playlist'), `${shown.value.length} made for you`].join(' · '))
const conflictOf = (p: Playlist) => conflicts.value.find((c) => c.playlistId === p.id)

// ---- Your playlists: new, and the menu (shared with a playlist's page)
const { renaming, asking, covering, spotifyUpdate, act, rename, confirm, saveCover } = usePlaylistActions(load)
// A Spotify playlist from its link: it opens once added.
const addingSpotify = ref(false)
async function addedSpotify(id: number) {
  await load()
  router.push(`/playlist/${id}`)
}
async function create() {
  const made = await library.createPlaylist('New playlist')
  await load()
  renaming.value = made.id
}

// ---- Made for you
const AUTO_MENU = coverMenu({ kind: 'auto' }, isDesktop)
async function actAuto(a: AutoPlaylist, action: CoverAction) {
  if (action === 'hide') autos.value = await library.setAutoPlaylist(a.source, { hidden: true })
  else if (action === 'export') await exportList(a.source, a.name.replace(': ', ', '))
  else await listAction(a.source, action)
}

// ---- A playlist waiting for a choice
const choosing = ref<PlaylistConflict | null>(null)
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader title="Playlists" :subtitle="subtitle" />

    <!-- Synced playlists changed on two devices that don't agree (one deleted here has no card). -->
    <SettingsTip v-for="c in conflicts" :key="c.syncId" kind="bad" class="mb-3">
      <span class="flex flex-wrap items-center justify-between gap-2">
        “{{ c.name }}” changed on this device and on {{ c.deviceName }} in ways that don’t agree.
        <Button size="sm" variant="outline" @click="choosing = c">Choose…</Button>
      </span>
    </SettingsTip>

    <section class="mb-10">
      <div class="mb-2 flex items-center gap-2">
        <h2 class="text-xl font-semibold">Your playlists</h2>
        <!-- Its two ways to add one, together. -->
        <div class="flex items-center">
          <Button variant="ghost" size="icon-sm" aria-label="New playlist" title="New playlist" @click="create"><Plus class="text-muted-foreground" /></Button>
          <Button variant="ghost" size="icon-sm" aria-label="Add from Spotify" title="Add from Spotify" @click="addingSpotify = true">
            <AudioLines class="text-muted-foreground" />
          </Button>
        </div>
      </div>
      <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
      <Transition name="part" mode="out-in">
        <div v-if="playlists.length" class="cover-grid [--cover-min:13rem]">
          <CoverCard
            v-for="p in playlists"
            :key="p.id"
            :title="p.name"
            :subtitle="playlistLine(p)"
            v-bind="playlistCover(p)"
            :menu="coverMenu(playlistKind(p, sync.available), isDesktop)"
            :editing="renaming === p.id"
            @open="conflictOf(p) ? (choosing = conflictOf(p)!) : router.push(`/playlist/${p.id}`)"
            @action="(action) => act(p, action as CoverAction)"
            @rename="(name) => rename(p, name)"
            @cancel="renaming = null"
          >
            <template #badge>
              <CoverBadge v-if="conflictOf(p)" :icon="CircleAlert" label="Needs a choice" open />
              <CoverBadge v-else-if="playlistBadge(p, sync.available)" v-bind="playlistBadge(p, sync.available)!" />
            </template>
          </CoverCard>
        </div>
        <EmptyState v-else-if="problem" :icon="CircleAlert" title="Can’t read your playlists" :text="problem" problem small>
          <Button size="sm" variant="outline" @click="load">Try again</Button>
        </EmptyState>
        <EmptyState v-else :icon="ListMusic" title="No playlists yet" text="Make one to keep songs together, in the order you like." small>
          <Button size="sm" @click="create">New playlist</Button>
        </EmptyState>
      </Transition>
      <p v-if="deleted.length" class="mt-4 text-sm text-muted-foreground">
        <RouterLink to="/settings" class="hover:text-foreground hover:underline">Recently deleted ({{ deleted.length }})</RouterLink>: kept for 30 days, in Settings.
      </p>
    </section>

    <section v-if="shown.length">
      <h2 class="mb-2 text-xl font-semibold">Made for you</h2>
      <div class="cover-grid [--cover-min:10.5rem]">
        <CoverCard
          v-for="a in shown"
          :key="a.source"
          v-bind="autoCard(a)"
          :menu="AUTO_MENU"
          @open="router.push(autoPage(a))"
          @action="(action) => actAuto(a, action as CoverAction)"
        />
      </div>
    </section>

    <PlaylistDialogs v-model="asking" @confirm="confirm" />
    <!-- Update from Spotify on a long one that changed there: its CSV's steps. -->
    <SpotifyUpdateDialog v-if="spotifyUpdate" :open="true" :playlist="spotifyUpdate" @update:open="(open) => !open && (spotifyUpdate = null)" />
    <SpotifyAddDialog v-model:open="addingSpotify" @added="addedSpotify" />
      <CoverCropDialog
        v-if="covering"
        :name="covering.name"
        :open="!!covering"
        @update:open="covering = null"
        @save="(picture) => saveCover(covering!, picture)"
      />
    <ConflictDialog v-if="choosing" :conflict="choosing" :open="!!choosing" @update:open="choosing = null" @answered="load" />
  </div>
</template>
