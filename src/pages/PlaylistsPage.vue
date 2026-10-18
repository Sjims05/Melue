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
import { CircleAlert, Plus } from '@lucide/vue'
import { computed, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { CoverBadge, CoverCard, coverMenu, type CoverAction } from '@/components/covers'
import { PageHeader } from '@/components/page'
import { SettingsTip } from '@/components/settings'
import { Button } from '@/components/ui/button'
import { library, type AutoPlaylist, type DeletedLinkedPlaylist, type Playlist } from '@/library'
import { librarySync, sync, type DeletedPlaylist, type PlaylistConflict } from '@/library/sync'
import { count } from '@/lib/format'
import { isDesktop } from '@/platform/platform'
import { exportList, listAction } from '@/player/songs'
import { usePlaylistActions } from './playlists/actions'
import ConflictDialog from './playlists/ConflictDialog.vue'
import PlaylistDialogs from './playlists/PlaylistDialogs.vue'
import { autoCard, autoPage, playlistBadge, playlistCover, playlistKind, playlistLine } from './playlists/playlists'

const router = useRouter()

const playlists = ref<Playlist[]>([])
const autos = ref<AutoPlaylist[]>([])
const deleted = ref<(DeletedPlaylist | DeletedLinkedPlaylist)[]>([])
const conflicts = ref<PlaylistConflict[]>([])
async function load() {
  ;[playlists.value, autos.value, deleted.value, conflicts.value] = await Promise.all([
    library.playlists().catch(() => []),
    library.autoPlaylists().catch(() => []),
    Promise.all([librarySync.deletedPlaylists().catch(() => []), library.deletedLinkedPlaylists().catch(() => [])]).then((lists) => lists.flat()),
    librarySync.conflicts().catch(() => []),
  ])
}
load()
const stops = [
  library.onChange((change) => change.reason !== 'likes' && load()),
  librarySync.onConflicts((list) => (conflicts.value = list)),
]
onUnmounted(() => stops.forEach((stop) => stop.then((unlisten) => unlisten())))

const shown = computed(() => autos.value.filter((a) => !a.hidden))
const subtitle = computed(() => [count(playlists.value.length, 'playlist'), `${shown.value.length} made for you`].join(' · '))
const conflictOf = (p: Playlist) => conflicts.value.find((c) => c.playlistId === p.id)

// ---- Your playlists: new, and the menu (shared with a playlist's page)
const { renaming, asking, act, rename, confirm } = usePlaylistActions(load)
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
        <Button variant="ghost" size="icon-sm" aria-label="New playlist" title="New playlist" @click="create"><Plus /></Button>
      </div>
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
            <CoverBadge v-else-if="playlistBadge(p)" v-bind="playlistBadge(p)!" />
          </template>
        </CoverCard>
      </div>
      <p v-else class="text-muted-foreground">No playlists yet: + makes one.</p>
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
    <ConflictDialog v-if="choosing" :conflict="choosing" :open="!!choosing" @update:open="choosing = null" @answered="load" />
  </div>
</template>
