<script setup lang="ts">
/**
 * A music folder (Melue's design, opened on top): the folder cover, "From the share" on the synced one, its name and
 * line ("Folder · 33 songs · 22 min"), Play and Shuffle; then its songs by where they are in it, each subfolder under
 * its own heading ("Album/CD1"). A big folder (all your music) scrolls like the Songs tab: only what's on screen is
 * drawn. It follows the library.
 */
import { Cloud, Folder } from '@lucide/vue'
import { computed, onUnmounted, ref } from 'vue'
import { useRoute, type RouteLocationNormalized } from 'vue-router'
import { AUTO_COVERS, coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, ListHeader } from '@/components/page'
import { SettingsBadge } from '@/components/settings'
import { VirtualSongList, type Song } from '@/components/songs'
import { library, type MusicFolder, type Source, type Track } from '@/library'
import { count, totalLength } from '@/lib/format'
import { isDesktop } from '@/platform/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { listAction, playList, playSong, songAction, trackSong } from '@/player/songs'
import { goBack, setPageTitle } from '@/router'
import { Button } from '@/components/ui/button'

const route = useRoute()
// This page's own address and folder (the route moves on when another page opens on top of it).
const here = { fullPath: route.fullPath } as RouteLocationNormalized
const id = Number(route.params.id)
const source: Source = `folder:${id}`

// undefined while it loads; null: no such folder (any more).
const folder = ref<MusicFolder | null>()
const tracks = ref<Track[]>([])
async function load() {
  const [folders, found] = await Promise.all([library.folders().catch(() => []), library.folderTracks(id).catch(() => [])])
  folder.value = folders.find((f) => f.id === id) ?? null
  tracks.value = found
  if (folder.value) setPageTitle(here, folder.value.name)
}
load()
const stop = library.onChange((change) => (change.sources.includes(source) || change.reason === 'folders') && load())
onUnmounted(() => stop.then((unlisten) => unlisten()))

const songs = computed(() => tracks.value.map(trackSong))
const paths = computed(() => new Map(tracks.value.map((t) => [t.id, t.path])))
const length = computed(() => tracks.value.reduce((sum, t) => sum + (t.durationMs ?? 0), 0))
// A file's subfolder ("Album/CD1"; its own files: none). Android's folders give addresses, not paths: no headings.
const headingOf = computed(() =>
  folder.value?.kind === 'path'
    ? (song: Song) => {
        const path = paths.value.get(Number(song.id)) ?? ''
        const cut = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
        return cut < 0 ? '' : path.slice(0, cut).replace(/\\/g, '/')
      }
    : undefined,
)

// The heading's right-click menu: the folder row's.
const MENU = coverMenu({ kind: 'list' })

</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <template v-if="folder">
      <ListHeader
        :title="folder.name"
        :colors="AUTO_COVERS.folder.colors"
        :icon="AUTO_COVERS.folder.icon"
        :empty="!songs.length"
        :menu="MENU"
        @action="(action) => listAction(source, action as CoverAction)"
        @play="playList(source)"
        @shuffle="playList(source, { shuffle: true })"
      >
        <template v-if="folder.synced" #badges><SettingsBadge class="gap-1"><Cloud class="size-3" />From the share</SettingsBadge></template>
        <template #line>
          Folder · {{ folder.enabled ? `${count(songs.length, 'song')} · ${totalLength(length)}` : 'Switched off: its songs are hidden (Settings)' }}
        </template>
      </ListHeader>
      <VirtualSongList
        v-if="songs.length"
        :songs="songs"
        :heading-of="headingOf"
        :current-id="current?.id"
        :playing="!!nowPlaying.state?.isPlaying"
        :place="{ showInFolder: isDesktop }"
        @play="playSong"
        @pause="togglePlay"
        @like="(song, liked) => like(Number(song.id), liked)"
        @action="songAction"
      />
    </template>
    <EmptyState v-else-if="folder === null" :icon="Folder" title="This folder isn’t in your library" text="It may have been removed from your music folders.">
      <Button size="sm" variant="outline" @click="goBack">Back</Button>
    </EmptyState>
  </div>
</template>
