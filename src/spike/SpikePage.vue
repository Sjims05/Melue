<script setup lang="ts">
/**
 * The frame of a spike test page: the sections every platform has (SMB, storage, wake lock,
 * the library, the log), with the platform's own sections in the slot between them.
 */
import {
  log,
  releaseWakeLock,
  requestWakeLock,
  smbDownloadOneFile,
  smbListFiles,
  logMsg,
} from './spike'
import { onMounted, onUnmounted, ref } from 'vue'
import {
  coverUrl,
  library,
  type Album,
  type Artist,
  type FolderCovers,
  type MusicFolder,
  type ScanSummary,
  type Track,
} from '@/library'
import { isAndroid, storage } from '@/platform'

defineProps<{ title: string }>()

const folders = ref<MusicFolder[]>([])
const songs = ref<Track[] | null>(null)
const albums = ref<Album[] | null>(null)
const artists = ref<Artist[] | null>(null)
const folderCovers = ref<FolderCovers | null>(null)

const describe = (s: ScanSummary) =>
  `${s.added} added, ${s.changed} changed, ${s.missing} missing, ${s.returned} back, ${s.moved} moved, ${s.unchanged} unchanged`

// Scans run at start-up and (Android) after changes too: log them all.
let stopScanLog: (() => void) | null = null
onMounted(async () => {
  stopScanLog = await library.onScan((event) => {
    if (event.phase === 'folder') {
      logMsg(event.error ? `Scan: ${event.name} skipped: ${event.error}` : `Scan: ${event.name}: ${describe(event.summary!)}`)
    } else if (event.phase === 'tags') {
      // Every batch of 50 sends one: log the first, every 500th and the last.
      if (event.done <= 50 || event.done % 500 === 0 || event.done === event.total) {
        logMsg(`Reading tags: ${event.done} of ${event.total}`)
      }
    } else if (event.phase === 'finished') {
      const s = event.summary
      const tags = s.tagsRead + s.unreadable ? `, tags read: ${s.tagsRead}, unreadable: ${s.unreadable}` : ''
      logMsg(`Scan finished${event.changed ? '' : ' (nothing changed)'}${tags}`)
      refreshFolders().catch(() => {})
      if (songs.value) showSongs().catch(() => {})
      if (albums.value) showAlbums().catch(() => {})
      if (artists.value) showArtists().catch(() => {})
    }
  })
  refreshFolders().catch(() => {})
  if (isAndroid) folderCovers.value = await library.folderCovers().catch(() => null)
})
onUnmounted(() => stopScanLog?.())

const scanNow = () =>
  run('Scan', async () => {
    const started = performance.now()
    const summary = await library.scan()
    logMsg(summary ? `Scan took ${Math.round(performance.now() - started)} ms` : 'Scan: one was running, it goes once more')
  })

async function refreshFolders() {
  folders.value = await library.folders()
}

async function run(label: string, action: () => Promise<unknown>) {
  try {
    await action()
  } catch (e) {
    logMsg(`${label} FAILED: ${e}`)
  }
  await refreshFolders().catch(() => {})
}

const addFolder = () =>
  run('Add folder', async () => {
    const location = await storage.pickMusicFolder()
    if (!location) return
    const folder = await library.addFolder(isAndroid ? 'android_tree' : 'path', location)
    logMsg(`Folder added: ${folder.name} (${folder.location})`)
  })

const useAllMusic = () =>
  run('All music on this phone', async () => {
    const folder = await library.addFolder('android_media', '')
    logMsg(`Folder added: ${folder.name}`)
  })

const musicPermission = () =>
  run('Music permission', async () => {
    const access = await library.musicAccess(true)
    logMsg(`Music permission: ${access.permission}${access.songs === null ? '' : `, ${access.songs} songs on the phone`}`)
  })

const toggleFolder = (folder: MusicFolder) =>
  run('Switch folder', async () => {
    const changed = await library.setFolderEnabled(folder.id, !folder.enabled)
    logMsg(`${changed.name}: ${changed.enabled ? 'on' : 'off'}`)
  })

// One list at a time: songs, albums or artists.
async function showSongs() {
  ;[albums.value, artists.value] = [null, null]
  songs.value = await library.tracks(0, 50)
}

async function showAlbums() {
  ;[songs.value, artists.value] = [null, null]
  albums.value = await library.albums(0, 50)
}

async function showArtists() {
  ;[songs.value, albums.value] = [null, null]
  artists.value = await library.artists(0, 100)
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`

const toggleFolderCovers = () =>
  run('Covers next to songs', async () => {
    folderCovers.value = await library.setFolderCovers(!folderCovers.value?.enabled)
    logMsg(`Covers next to songs: ${folderCovers.value.enabled ? 'on' : 'off'} (photos permission: ${folderCovers.value.permission})`)
  })

const duration = (ms: number | null) =>
  ms === null ? '' : `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, '0')}`

const details = (song: Track) =>
  [
    song.trackNo && `#${song.discNo ? `${song.discNo}.` : ''}${song.trackNo}`,
    song.year,
    song.genre,
    duration(song.durationMs),
    song.format,
    song.bitrateKbps && `${song.bitrateKbps} kbps`,
    song.sampleRate && `${song.sampleRate / 1000} kHz`,
  ]
    .filter(Boolean)
    .join(' · ')

const removeFolder = (folder: MusicFolder) =>
  run('Remove folder', async () => {
    await library.removeFolder(folder.id)
    logMsg(`Folder removed: ${folder.name}`)
  })

async function libraryStatus() {
  try {
    const status = await library.status()
    logMsg(
      `Library: schema version ${status.schemaVersion} | ${status.folders} folders, ${status.tracks} tracks, ` +
        `${status.albums} albums, ${status.artists} artists, ${status.playlists} playlists`,
    )
  } catch (e) {
    logMsg(`Library FAILED: ${e}`)
  }
}
</script>

<template>
  <div class="p-4 space-y-6 max-w-xl mx-auto">
    <h1 class="text-xl font-bold">{{ title }}</h1>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">1. SMB — connect, list, download</h2>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="smbListFiles">List files</button>
        <button class="border rounded px-3 py-1" @click="smbDownloadOneFile">Download one file</button>
      </div>
    </section>


    <slot />

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">5. Screen wake lock</h2>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="requestWakeLock">Request wake lock</button>
        <button class="border rounded px-3 py-1" @click="releaseWakeLock">Release wake lock</button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">6. Library</h2>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="libraryStatus">Library status</button>
        <button class="border rounded px-3 py-1" @click="run('Folders', refreshFolders)">Show folders</button>
        <button class="border rounded px-3 py-1" @click="addFolder">Add folder</button>
        <button class="border rounded px-3 py-1" @click="scanNow">Scan now</button>
        <button class="border rounded px-3 py-1" @click="run('Songs', showSongs)">Show songs</button>
        <button class="border rounded px-3 py-1" @click="run('Albums', showAlbums)">Show albums</button>
        <button class="border rounded px-3 py-1" @click="run('Artists', showArtists)">Show artists</button>
        <template v-if="isAndroid">
          <button class="border rounded px-3 py-1" @click="useAllMusic">Use all music on this phone</button>
          <button class="border rounded px-3 py-1" @click="musicPermission">Music permission</button>
          <button class="border rounded px-3 py-1" @click="toggleFolderCovers">
            Covers next to songs: {{ folderCovers?.enabled ? 'on' : 'off' }}
          </button>
        </template>
      </div>
      <ul class="text-sm space-y-1">
        <li v-for="folder in folders" :key="folder.id" class="flex flex-wrap items-center gap-2">
          <span :class="{ 'opacity-50': !folder.enabled }">{{ folder.name }} ({{ folder.tracks }} songs)</span>
          <span class="text-xs text-muted-foreground break-all">{{ folder.kind === 'android_media' ? '' : folder.location }}</span>
          <button class="border rounded px-2 text-xs" @click="toggleFolder(folder)">{{ folder.enabled ? 'Switch off' : 'Switch on' }}</button>
          <button class="border rounded px-2 text-xs" @click="removeFolder(folder)">Remove</button>
        </li>
      </ul>
      <div v-if="songs" class="space-y-1">
        <div class="text-xs text-muted-foreground">The first {{ songs.length }} songs by title</div>
        <div v-for="song in songs" :key="song.id" class="flex items-center gap-2 text-sm">
          <img v-if="song.cover" :src="coverUrl(song.cover)" class="w-10 h-10 rounded object-cover shrink-0" alt="" />
          <div v-else class="w-10 h-10 rounded bg-muted shrink-0" />
          <div class="min-w-0">
            <div class="truncate">{{ song.title }}</div>
            <div class="truncate text-xs text-muted-foreground">
              {{ [song.artist, song.album].filter(Boolean).join(' — ') }}
              <span v-if="song.albumArtist && song.albumArtist !== song.artist">(album: {{ song.albumArtist }})</span>
            </div>
            <div class="truncate text-xs text-muted-foreground">
              <template v-if="song.tagsError">Tags unreadable: {{ song.tagsError }}</template>
              <template v-else-if="!song.tagsRead">Tags not read yet</template>
              <template v-else>{{ details(song) }}</template>
            </div>
          </div>
        </div>
      </div>
      <div v-if="albums" class="space-y-1">
        <div class="text-xs text-muted-foreground">The first {{ albums.length }} albums by title</div>
        <div v-for="album in albums" :key="album.id" class="flex items-center gap-2 text-sm">
          <img v-if="album.cover" :src="coverUrl(album.cover)" class="w-10 h-10 rounded object-cover shrink-0" alt="" />
          <div v-else class="w-10 h-10 rounded bg-muted shrink-0" />
          <div class="min-w-0">
            <div class="truncate">{{ album.title }}</div>
            <div class="truncate text-xs text-muted-foreground">
              {{ album.artist ?? 'Various artists' }}{{ album.year ? ` · ${album.year}` : '' }} ·
              {{ plural(album.tracks, 'song') }}
            </div>
          </div>
        </div>
      </div>
      <div v-if="artists" class="space-y-1">
        <div class="text-xs text-muted-foreground">The first {{ artists.length }} artists by name</div>
        <div v-for="artist in artists" :key="artist.id" class="text-sm">
          {{ artist.name }}
          <span class="text-xs text-muted-foreground">
            {{ plural(artist.tracks, 'song') }}{{ artist.featured ? `, featured on ${artist.featured}` : ''
            }}{{ artist.albums ? `, ${plural(artist.albums, 'album')}` : '' }}
          </span>
        </div>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">Log</h2>
      <div class="text-xs font-mono max-h-96 overflow-y-auto space-y-1">
        <div v-for="(line, i) in log" :key="i">{{ line }}</div>
      </div>
    </section>
  </div>
</template>
