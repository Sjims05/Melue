<script setup lang="ts">
/**
 * One of your playlists (Melue's design, opened on top): its covers, badges (Synced / This device / Linked, the
 * file's name, File gone), its name (click it to rename), "Playlist · 12 songs · 48 min", Play, Shuffle and ⋯ (its
 * card's menu, also on right-clicking the heading). Then its songs in order: drag to move, ✕ to take out. Missing songs stay greyed at their place, as do
 * songs another device added that aren't on this device yet; a linked playlist's songs new in the file or removed here
 * are greyed with Add / Restore. It follows the library.
 */
import { CircleAlert, FileText, ListMusic } from '@lucide/vue'
import { computed, nextTick, onUnmounted, ref } from 'vue'
import { useRoute, type RouteLocationNormalized } from 'vue-router'
import { coverMenu, type CoverAction } from '@/components/covers'
import { EmptyState, ListHeader } from '@/components/page'
import { SettingsBadge, SettingsTip } from '@/components/settings'
import { FileOff, NOT_HERE, SongList, SpotifyOff, type Song, type SongAction } from '@/components/songs'
import { Button } from '@/components/ui/button'
import { library, type Playlist, type PlaylistEntry, type Source } from '@/library'
import { librarySync, sync, type PlaylistConflict } from '@/library/sync'
import { isDesktop } from '@/platform/platform'
import { current, like, nowPlaying, togglePlay } from '@/player/nowPlaying'
import { playList, playSong, songAction, trackSong } from '@/player/songs'
import { goBack, setPageTitle } from '@/router'
import { usePlaylistActions } from './playlists/actions'
import ConflictDialog from './playlists/ConflictDialog.vue'
import CoverCropDialog from './playlists/CoverCropDialog.vue'
import PlaylistDialogs from './playlists/PlaylistDialogs.vue'
import SpotifyUpdateDialog from './playlists/SpotifyUpdateDialog.vue'
import FindSongDialog from './playlists/FindSongDialog.vue'
import { playlistCover, playlistKind, playlistLine, playlistLink, playlistWhere, readSpotify } from './playlists/playlists'
import { onLibraryChange } from '@/library/changes'

const route = useRoute()
// This page's own address and playlist (the route moves on when another page opens on top of it).
const here = { fullPath: route.fullPath } as RouteLocationNormalized
const id = Number(route.params.id)
const source: Source = `playlist:${id}`

// undefined while it loads; null: no such playlist (any more).
const playlist = ref<Playlist | null>()
const entries = ref<PlaylistEntry[]>([])
const conflict = ref<PlaylistConflict | null>(null)
// Made from a song's menu (?rename=1): its name ready to type once it's shown.
let renameFirst = route.query.rename === '1'
async function load() {
  const [found, list, conflicts] = await Promise.all([
    library.playlist(id).catch(() => null),
    library.playlistEntries(id).catch(() => []),
    librarySync.conflicts().catch(() => []),
  ])
  playlist.value = found
  entries.value = list
  conflict.value = conflicts.find((c) => c.playlistId === id) ?? null
  if (found) setPageTitle(here, found.name)
  if (found && renameFirst) {
    renameFirst = false
    await nextTick()
    header.value?.rename()
  }
}
// A Spotify playlist: read again (unless it was a few minutes ago), so what's shown is as Spotify has it now.
load().then(() => playlist.value?.spotify && readSpotify(id).catch(() => {}))
// Its own changes (not other playlists'), and songs coming or going.
onLibraryChange((change) => change.reason !== 'likes' && (change.reason !== 'playlists' || change.sources.includes(source)), load)
const stops = [librarySync.onConflicts(() => load())]
onUnmounted(() => stops.forEach((stop) => stop.then((unlisten) => unlisten())))

// ---- Its songs: each entry a row (the same song twice: two rows), greyed ones at their place
// A linked playlist's songs that differ from its file get a note; one added here (not in the file) says "Only in Melue".
const STATE_NOTES = { suggested: 'New in the file', removed_here: 'Removed here', file_removed: 'Removed from the file' } as const
const fileName = (line: string | null) => line?.split(/[\\/]/).pop() ?? null
function entrySong(e: PlaylistEntry): Song {
  // Greyed by its state (not in Melue's version): its own id, so it never counts as the song playing.
  const left = e.state === 'suggested' || e.state === 'removed_here'
  const song: Song = e.track
    ? { ...trackSong(e.track), ...(left ? { id: `x${e.id}` } : {}) }
    : { id: `x${e.id}`, title: e.syncTitle ?? fileName(e.fileEntry) ?? 'Unknown song', artist: e.syncArtist ?? '', duration: 0 }
  const note =
    e.state !== 'both' && e.state !== 'here_only'
      ? STATE_NOTES[e.state]
      : !e.track
        ? e.syncTitle
          ? NOT_HERE
          : 'Not found'
        : e.state === 'here_only' && !playlist.value?.writeBack
          ? 'Only in Melue'
          : undefined
  // "Only in Melue": short of room, its icon (not on Spotify, or not in the file).
  const noteIcon = note === 'Only in Melue' ? (playlist.value?.spotify ? SpotifyOff : FileOff) : undefined
  return { ...song, key: `e${e.id}`, note, noteIcon, restore: e.state === 'suggested' ? 'Add' : e.state === 'removed_here' ? 'Restore' : undefined }
}
// A linked playlist's songs added in Melue that its file doesn't have (not written back to it): how many, for its line.
const onlyInMelue = computed(() =>
  playlist.value?.kind === 'linked' && playlist.value.following && !playlist.value.writeBack
    ? entries.value.filter((e) => e.state === 'here_only').length
    : 0,
)
// Unfollowed: only its own version (what it differs from the file shows again once it follows the file again).
const shown = computed(() =>
  playlist.value?.kind === 'linked' && !playlist.value.following ? entries.value.filter((e) => e.state !== 'suggested' && e.state !== 'removed_here') : entries.value,
)
const songs = computed(() =>
  shown.value.map((e) => {
    const song = entrySong(e)
    return playlist.value?.following === false && e.track ? { ...song, note: undefined } : song
  }),
)
const greyed = computed(() =>
  shown.value.filter((e) => !e.track || !e.track.available || e.state === 'suggested' || e.state === 'removed_here').map((e) => entrySong(e).id),
)
const entryOf = (song: Song) => entries.value.find((e) => `e${e.id}` === song.key)

// Taken out (✕ or the menu) and moved: shown at once, then done. A song of a followed file's stays, greyed ("Removed
// here"): it fades to grey in its place instead of folding away.
const following = computed(() => playlist.value?.kind === 'linked' && playlist.value.following)
const staysWhenRemoved = (song: Song) => following.value && ['both', 'file_removed'].includes(entryOf(song)?.state ?? '')
async function remove(song: Song) {
  const entry = entryOf(song)
  if (!entry) return
  if (staysWhenRemoved(song)) entries.value = entries.value.map((e) => (e.id === entry.id ? { ...e, state: 'removed_here' } : e))
  else entries.value = entries.value.filter((e) => e.id !== entry.id)
  await library.removeFromPlaylist(id, [entry.id])
  await load()
}
// Places in the rows shown, to places among all its entries (an unfollowed one hides some).
async function move(from: number, to: number) {
  const entry = shown.value[from]
  const position = entries.value.indexOf(shown.value[to])
  const list = entries.value.filter((e) => e !== entry)
  list.splice(position, 0, entry)
  entries.value = list
  await library.moveInPlaylist(id, entry.id, position)
  await load()
}
// Find song: one of your songs for one that isn't here (FindSongDialog.vue).
const finding = ref<{ entry: number; title: string; artist: string } | null>(null)
async function act(song: Song, action: SongAction) {
  const entry = entryOf(song)
  if (action === 'findSong' && entry) finding.value = { entry: entry.id, title: song.title, artist: song.artist }
  else if (action === 'restore' && entry) {
    await library.restorePlaylistEntry(id, entry.id)
    await load()
  } else if (action === 'removeFromPlaylist') await remove(song)
  else await songAction(song, action)
}

// ---- The playlist itself: rename (its name, or the menu), the rest of its menu
const header = ref<InstanceType<typeof ListHeader> | null>(null)
const { asking, covering, spotifyUpdate, act: actOnPlaylist, rename, confirm, saveCover } = usePlaylistActions(load, goBack)
const menu = computed(() => (playlist.value ? coverMenu(playlistKind(playlist.value, sync.available), isDesktop) : []))
// Its badges: where it lives (a linked one: where its file is), and a linked one's link.
const badges = computed(() => (playlist.value ? [playlistWhere(playlist.value, sync.available), playlistLink(playlist.value)].filter((b) => !!b) : []))
function pick(action: CoverAction) {
  if (!playlist.value) return
  if (action === 'rename') header.value?.rename()
  else actOnPlaylist(playlist.value, action)
}
const choosing = ref(false)
// A Spotify playlist with more than 100 songs: its full list from a CSV ("Update list"; the "!" when it changed there).
const long = computed(() => (playlist.value?.spotify?.total ?? 0) > 100)
const updatingSpotify = computed({ get: () => !!spotifyUpdate.value, set: (open) => (spotifyUpdate.value = open ? (playlist.value ?? null) : null) })
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <template v-if="playlist">
      <ListHeader
        ref="header"
        :title="playlist.name"
        v-bind="playlistCover(playlist, 'large')"
        cover-label="Change cover"
        @cover="covering = playlist"
        :empty="!playlist.available"
        editable
        :menu="menu"
        @action="(action) => pick(action as CoverAction)"
        @rename="(name) => rename(playlist!, name)"
        @play="playList(source)"
        @shuffle="playList(source, { shuffle: true })"
      >
        <template #badges>
          <SettingsBadge v-for="badge in badges" :key="badge.label" class="h-7 gap-1.5 px-2.5 text-[13px]">
            <component :is="badge.icon" class="size-3.5" />{{ badge.label }}
          </SettingsBadge>
          <SettingsBadge v-if="playlist.file && !playlist.spotify" class="h-7 gap-1.5 px-2.5 text-[13px]"><FileText class="size-3.5" />{{ playlist.file }}</SettingsBadge>
          <SettingsBadge v-if="playlist.fileMissing" class="h-7 gap-1.5 px-2.5 text-[13px] text-destructive"><CircleAlert class="size-3.5" />File gone</SettingsBadge>
        </template>
        <template #line>Playlist · {{ playlistLine(playlist) }}{{ onlyInMelue ? ` · ${onlyInMelue} only in Melue` : '' }}</template>
      </ListHeader>

      <Transition name="drop">
        <SettingsTip v-if="long" :kind="playlist.spotify?.changed ? 'bad' : undefined" class="mb-6">
          <span class="flex flex-wrap items-center justify-between gap-2">
            <span>
              {{ playlist.spotify?.changed ? 'Changed on Spotify since this list was updated. ' : '' }}Spotify only shares the first 100 of its
              {{ playlist.spotify?.total }} songs by itself: the rest come from a CSV.
            </span>
            <Button size="sm" variant="outline" @click="updatingSpotify = true">Update list</Button>
          </span>
        </SettingsTip>
      </Transition>
      <Transition name="drop">
        <SettingsTip v-if="conflict" kind="bad" class="mb-6">
          <span class="flex flex-wrap items-center justify-between gap-2">
            It changed on this device and on {{ conflict.deviceName }} in ways that don’t agree.
            <Button size="sm" variant="outline" @click="choosing = true">Choose…</Button>
          </span>
        </SettingsTip>
      </Transition>

      <!-- What it shows changing (the list, nothing yet, a problem): a fade. -->
      <Transition name="part" mode="out-in">
        <SongList
          v-if="songs.length"
          :songs="songs"
          reorderable
          :keep-on-remove="staysWhenRemoved"
          :unavailable="greyed"
          :current-id="current?.id"
          :playing="!!nowPlaying.state?.isPlaying"
          :place="{ inPlaylist: true, showInFolder: isDesktop }"
          @play="playSong"
          @pause="togglePlay"
          @like="(song, liked) => like(Number(song.id), liked)"
          @remove="(song) => remove(song)"
          @move="move"
          @action="act"
        />
        <EmptyState
          v-else
          :icon="ListMusic"
          title="No songs in this playlist yet"
          :text="`Add songs with a song’s ⋯ → Add to playlist${isDesktop ? ', or drop music files here' : ''}.`"
          small
        >
          <Button size="sm" variant="outline" @click="$router.push('/library')">Go to Library</Button>
        </EmptyState>
      </Transition>

      <PlaylistDialogs v-model="asking" @confirm="confirm" />
      <SpotifyUpdateDialog v-if="playlist.spotify" v-model:open="updatingSpotify" :playlist="playlist" />
      <FindSongDialog
        :open="!!finding"
        :playlist="id"
        :entry="finding?.entry ?? null"
        :title="finding?.title ?? ''"
        :artist="finding?.artist ?? ''"
        @update:open="(open) => !open && (finding = null)"
        @chosen="load"
      />
      <CoverCropDialog
        v-if="covering"
        :name="covering.name"
        :open="!!covering"
        @update:open="covering = null"
        @save="(picture) => saveCover(covering!, picture)"
      />
      <ConflictDialog v-if="conflict" v-model:open="choosing" :conflict="conflict" @answered="load" />
    </template>
    <EmptyState v-else-if="playlist === null" :icon="ListMusic" title="This playlist isn’t in your library" text="It may have been deleted: Settings keeps deleted ones for a while.">
      <Button size="sm" variant="outline" @click="goBack">Back</Button>
    </EmptyState>
  </div>
</template>
