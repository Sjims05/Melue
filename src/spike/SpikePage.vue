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
  type Playlist,
  type PlaylistEntry,
  type QueueFollow,
  type ScanSummary,
  type Source,
  type Track,
} from '@/library'
import { playSource, setLiked } from '@/library/playback'
import {
  describeSetupError,
  share,
  sync,
  type PasswordStore,
  type SetupError,
  type ShareInfo,
  type ShareSetup,
  type SyncFolderChoice,
} from '@/library/sync'
import { smbTestLogin } from './spike'
import { isAndroid, storage } from '@/platform'

defineProps<{ title: string }>()

const folders = ref<MusicFolder[]>([])
const songs = ref<Track[] | null>(null)
const albums = ref<Album[] | null>(null)
const artists = ref<Artist[] | null>(null)
const playlists = ref<Playlist[] | null>(null)
const openPlaylist = ref<Playlist | null>(null)
const entries = ref<PlaylistEntry[]>([])
const playlistName = ref('')
const folderCovers = ref<FolderCovers | null>(null)

const describe = (s: ScanSummary) =>
  `${s.added} added, ${s.changed} changed, ${s.missing} missing, ${s.returned} back, ${s.moved} moved, ${s.unchanged} unchanged`

// Scans run at start-up and (Android) after changes too: log them all, and what they changed.
let stopScanLog: (() => void) | null = null
let stopChangeLog: (() => void) | null = null
onMounted(async () => {
  stopChangeLog = await library.onChange((change) => {
    const sources = change.sources.length > 6 ? `${change.sources.slice(0, 6).join(', ')}, ...` : change.sources.join(', ')
    logMsg(`Library changed (${change.reason}): ${change.tracks.length} songs${sources ? `; lists: ${sources}` : ''}`)
    refreshFolders().catch(() => {})
    if (songs.value) showSongs().catch(() => {})
    if (albums.value) showAlbums().catch(() => {})
    if (artists.value) showArtists().catch(() => {})
    if (playlists.value) showPlaylists().catch(() => {})
    if (openPlaylist.value) refreshPlaylist().catch(() => {})
  })
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
    }
  })
  refreshFolders().catch(() => {})
  queueFollow.value = await library.queueFollow().catch(() => null)
  shareInfo.value = await share.info().catch(() => null)
  if (isAndroid) folderCovers.value = await library.folderCovers().catch(() => null)
})
onUnmounted(() => {
  stopScanLog?.()
  stopChangeLog?.()
})

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
    logMsg('Music permission: asking…')
    const access = await library.musicAccess(true)
    const hint = access.permission === 'denied' ? " (Android won't ask again: Settings → Apps → Melue → Permissions → Music and audio)" : ''
    logMsg(`Music permission: ${access.permission}${hint}${access.songs === null ? '' : `, ${access.songs} songs on the phone`}`)
  })

const toggleFolder = (folder: MusicFolder) =>
  run('Switch folder', async () => {
    const changed = await library.setFolderEnabled(folder.id, !folder.enabled)
    logMsg(`${changed.name}: ${changed.enabled ? 'on' : 'off'}`)
  })

// One list at a time: songs, albums or artists.
// Songs: all by title, recently played, or most played.
const songsKind = ref<'all' | 'recent' | 'most'>('all')
const songsHeading = { all: 'songs by title', recent: 'recently played songs', most: 'most played songs' }

async function showSongs(kind = songsKind.value) {
  ;[albums.value, artists.value] = [null, null]
  songsKind.value = kind
  songs.value = await (kind === 'recent' ? library.recentlyPlayed(50) : kind === 'most' ? library.mostPlayed(50) : library.tracks(0, 50))
}

// ---- playing (the player's own controls are in the playback section)

const play = (source: Source, startAt?: number) =>
  run('Play', async () => {
    await playSource(source, { startAt })
    logMsg(`Playing ${source}${startAt ? ` from song ${startAt}` : ''}`)
  })

const toggleLike = (song: Track) =>
  run('Like', async () => {
    await setLiked(song.id, !song.liked)
    logMsg(`${song.liked ? 'Unliked' : 'Liked'} ${song.title}`)
  })

// ---- the share (Phase 2): set up from .env's test login, or (desktop) a network drive

const shareForm = ref({ ...smbTestLogin, passwordStore: 'secure' as PasswordStore, drivePath: '', streamDrive: false })
const shareInfo = ref<ShareInfo | null>(null)
/** A setup refused for want of a secure store: offers asking each time or a plain file. */
const noSecureStore = ref(false)

const smbSetup = (): ShareSetup => ({
  kind: 'smb',
  host: shareForm.value.host,
  share: shareForm.value.share,
  folder: shareForm.value.folder,
  user: shareForm.value.user,
  passwordStore: shareForm.value.passwordStore,
})

async function shareRun(label: string, action: () => Promise<unknown>) {
  try {
    await action()
  } catch (e) {
    noSecureStore.value = (e as SetupError)?.kind === 'noSecureStore'
    logMsg(`${label} FAILED: ${describeSetupError(e)}`)
  }
  shareInfo.value = await share.info().catch(() => null)
  await refreshFolders().catch(() => {})
}

/** Android: the sync folder, chosen with the picker (pick or create Music/Melue). Desktop: Music/Melue. */
async function chooseSyncFolder(): Promise<SyncFolderChoice | undefined> {
  if (!isAndroid) return undefined
  const location = await storage.pickMusicFolder()
  if (!location) throw { kind: 'other', detail: 'no folder chosen' }
  return { kind: 'android_tree', location }
}

const testShare = () =>
  shareRun('Test connection', async () => {
    await share.test(smbSetup(), shareForm.value.pass)
    logMsg('Share: connection OK')
  })

const setUpShare = (store?: PasswordStore) =>
  shareRun('Set up share', async () => {
    if (store) shareForm.value.passwordStore = store
    noSecureStore.value = false
    const info = await share.setUp(smbSetup(), shareForm.value.pass, await chooseSyncFolder())
    logMsg(`Share set up: ${info.settings?.kind} · password kept: ${shareForm.value.passwordStore} · synced folder: ${info.syncFolder?.location}`)
  })

const setUpDrive = () =>
  shareRun('Set up drive', async () => {
    const stream = shareForm.value.streamDrive
    const info = await share.setUp({ kind: 'drive', path: shareForm.value.drivePath, stream })
    logMsg(`Network drive set up: ${stream ? 'played straight from it' : `copied to ${info.syncFolder?.location}`}`)
  })

const giveSharePassword = () =>
  shareRun('Password', async () => {
    await share.setPassword(shareForm.value.pass)
    logMsg('Share: password accepted')
  })

const removeShare = () =>
  shareRun('Remove share', async () => {
    await share.remove()
    logMsg('Share removed: its synced folder stays as a normal folder, with its songs')
  })

const showInOtherApps = (shown: boolean) =>
  shareRun('Show in other apps', async () => {
    await share.setShownInOtherApps(shown)
    logMsg(`Synced music ${shown ? 'shown in' : 'hidden from'} other apps`)
  })

const queueFollow = ref<QueueFollow | null>(null)
const toggleQueueFollow = () =>
  run('Queue follows library', async () => {
    const current = queueFollow.value ?? (await library.queueFollow())
    queueFollow.value = await library.setQueueFollow({ ...current, enabled: !current.enabled })
    logMsg(`The queue follows library changes: ${queueFollow.value.enabled ? 'on' : 'off'}`)
  })

async function showAlbums() {
  ;[songs.value, artists.value] = [null, null]
  albums.value = await library.albums(0, 50)
}

async function showArtists() {
  ;[songs.value, albums.value] = [null, null]
  artists.value = await library.artists(0, 100)
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`

// ---- playlists (the change event refreshes what's shown)

async function showPlaylists() {
  playlists.value = await library.playlists()
}

async function refreshPlaylist() {
  const open = openPlaylist.value
  if (!open) return
  openPlaylist.value = await library.playlist(open.id).catch(() => null)
  entries.value = openPlaylist.value ? await library.playlistEntries(open.id) : []
}

const createPlaylist = () =>
  run('Create playlist', async () => {
    const created = await library.createPlaylist(playlistName.value)
    playlistName.value = ''
    logMsg(`Playlist made: ${created.name}`)
    await showPlaylists()
  })

const openList = (playlist: Playlist) =>
  run('Open playlist', async () => {
    openPlaylist.value = playlist
    await refreshPlaylist()
  })

const renamePlaylist = (playlist: Playlist) =>
  run('Rename playlist', async () => {
    const renamed = await library.renamePlaylist(playlist.id, playlistName.value)
    playlistName.value = ''
    logMsg(`Playlist renamed: ${playlist.name} → ${renamed.name}`)
  })

const deletePlaylist = (playlist: Playlist) =>
  run('Delete playlist', async () => {
    await library.deletePlaylist(playlist.id)
    if (openPlaylist.value?.id === playlist.id) openPlaylist.value = null
    logMsg(`Playlist deleted: ${playlist.name}`)
  })

const addSong = (song: Track) =>
  run('Add to playlist', async () => {
    if (!openPlaylist.value) return
    await library.addToPlaylist(openPlaylist.value.id, [song.id])
    logMsg(`Added ${song.title} to ${openPlaylist.value.name}`)
  })

const moveEntry = (entry: PlaylistEntry, by: number) =>
  run('Move in playlist', async () => {
    if (openPlaylist.value) await library.moveInPlaylist(openPlaylist.value.id, entry.id, Math.max(0, entry.position + by))
  })

const removeEntry = (entry: PlaylistEntry) =>
  run('Remove from playlist', async () => {
    if (openPlaylist.value) await library.removeFromPlaylist(openPlaylist.value.id, [entry.id])
  })

// Linked playlists: greyed songs, settings, unlinking.
const greyed = (entry: PlaylistEntry) => entry.state === 'removed_here' || entry.state === 'suggested' || !entry.track?.available
const stateLabel: Record<string, string> = {
  both: '',
  here_only: 'only here',
  removed_here: 'removed here',
  suggested: 'suggested',
  file_removed: 'removed from file',
}

const restoreEntry = (entry: PlaylistEntry) =>
  run('Restore', async () => {
    if (openPlaylist.value) await library.restorePlaylistEntry(openPlaylist.value.id, entry.id)
  })

const toggleOption = (option: 'newInFile' | 'removedFromFile' | 'writeBack') =>
  run('Playlist setting', async () => {
    const p = openPlaylist.value
    if (!p) return
    const options =
      option === 'newInFile'
        ? { newInFile: p.newInFile === 'added' ? ('suggested' as const) : ('added' as const) }
        : option === 'removedFromFile'
          ? { removedFromFile: p.removedFromFile === 'removed' ? ('kept' as const) : ('removed' as const) }
          : { writeBack: !p.writeBack }
    openPlaylist.value = await library.setPlaylistOptions(p.id, options)
  })

const unlink = () =>
  run('Unlink', async () => {
    if (!openPlaylist.value) return
    openPlaylist.value = await library.unlinkPlaylist(openPlaylist.value.id)
    logMsg(`${openPlaylist.value.name} is an app playlist now`)
  })

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
      <h3 class="text-sm font-medium">Folders and scanning</h3>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="libraryStatus">Library status</button>
        <button class="border rounded px-3 py-1" @click="run('Folders', refreshFolders)">Show folders</button>
        <button class="border rounded px-3 py-1" @click="addFolder">Add folder</button>
        <button class="border rounded px-3 py-1" @click="scanNow">Scan now</button>
        <template v-if="isAndroid">
          <button class="border rounded px-3 py-1" @click="useAllMusic">Use all music on this phone</button>
          <button class="border rounded px-3 py-1" @click="musicPermission">Music permission</button>
          <button class="border rounded px-3 py-1" @click="toggleFolderCovers">
            Covers next to songs: {{ folderCovers?.enabled ? 'on' : 'off' }}
          </button>
        </template>
      </div>
      <h3 class="text-sm font-medium">Browsing</h3>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="run('Songs', () => showSongs('all'))">Songs</button>
        <button class="border rounded px-3 py-1" @click="run('Albums', showAlbums)">Albums</button>
        <button class="border rounded px-3 py-1" @click="run('Artists', showArtists)">Artists</button>
        <button class="border rounded px-3 py-1" @click="run('Songs', () => showSongs('recent'))">Recently played</button>
        <button class="border rounded px-3 py-1" @click="run('Songs', () => showSongs('most'))">Most played</button>
      </div>
      <h3 class="text-sm font-medium">Playing</h3>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="play('songs')">Play all songs</button>
        <button class="border rounded px-3 py-1" @click="play('liked')">Play liked</button>
        <button class="border rounded px-3 py-1" @click="toggleQueueFollow">
          Queue follows library: {{ queueFollow?.enabled === false ? 'off' : 'on' }}
        </button>
      </div>
      <h3 class="text-sm font-medium">Share setup</h3>
      <div class="text-sm">
        Sync available: {{ sync.available ? 'yes' : 'no' }}
        <template v-if="shareInfo?.settings">
          · {{ shareInfo.settings.kind === 'smb' ? `${shareInfo.settings.user}@${shareInfo.settings.host}/${shareInfo.settings.share}` : shareInfo.settings.path }}
          · synced folder: {{ shareInfo.syncFolder?.location ?? '?' }}
          <span v-if="shareInfo.needsPassword" class="font-medium">· password needed</span>
        </template>
        <template v-else-if="shareInfo?.defaultSyncFolder"> · synced music would go to {{ shareInfo.defaultSyncFolder }}</template>
      </div>
      <div class="flex flex-wrap gap-2 text-sm">
        <input v-model="shareForm.host" class="border rounded px-2 py-1 w-36" placeholder="Host" />
        <input v-model="shareForm.share" class="border rounded px-2 py-1 w-28" placeholder="Share" />
        <input v-model="shareForm.folder" class="border rounded px-2 py-1 w-36" placeholder="Music folder in it" />
        <input v-model="shareForm.user" class="border rounded px-2 py-1 w-28" placeholder="User" />
        <input v-model="shareForm.pass" type="password" class="border rounded px-2 py-1 w-28" placeholder="Password" />
        <select v-model="shareForm.passwordStore" class="border rounded px-2 py-1">
          <option value="secure">Password: secure store</option>
          <option value="ask">Password: ask each time</option>
          <option value="file">Password: plain file</option>
        </select>
      </div>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="testShare">Test connection</button>
        <button class="border rounded px-3 py-1" @click="setUpShare()">Set up share{{ isAndroid ? ' (choose folder)' : '' }}</button>
        <button class="border rounded px-3 py-1" @click="giveSharePassword">{{ shareInfo?.needsPassword ? 'Give password' : 'Change password' }}</button>
        <button class="border rounded px-3 py-1" @click="removeShare">Remove share</button>
        <template v-if="isAndroid">
          <button class="border rounded px-3 py-1" @click="showInOtherApps(true)">Show in other apps</button>
          <button class="border rounded px-3 py-1" @click="showInOtherApps(false)">Hide from other apps</button>
        </template>
      </div>
      <div v-if="noSecureStore" class="flex flex-wrap items-center gap-2 text-sm border rounded p-2">
        No secure place for the password on this system.
        <button class="border rounded px-3 py-1" @click="setUpShare('ask')">Ask each time</button>
        <button class="border rounded px-3 py-1" @click="setUpShare('file')">Keep it in a plain file</button>
      </div>
      <div v-if="!isAndroid" class="flex flex-wrap items-center gap-2 text-sm">
        <span class="text-xs text-muted-foreground">Or a network drive Windows has connected (tip: it can be played straight from):</span>
        <input v-model="shareForm.drivePath" class="border rounded px-2 py-1 w-40" placeholder="Z:\ or \\nas\music" />
        <label class="flex items-center gap-1"><input v-model="shareForm.streamDrive" type="checkbox" /> play straight from it</label>
        <button class="border rounded px-3 py-1" @click="setUpDrive">Set up drive</button>
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
        <div class="text-xs text-muted-foreground">The first {{ songs.length }} {{ songsHeading[songsKind] }}</div>
        <div v-for="song in songs" :key="song.id" class="flex items-center gap-2 text-sm">
          <button class="border rounded px-2 text-xs shrink-0" title="Play from here" @click="play('songs', song.id)">▶</button>
          <button class="border rounded px-2 text-xs shrink-0" :title="song.liked ? 'Unlike' : 'Like'" @click="toggleLike(song)">
            {{ song.liked ? '♥' : '♡' }}
          </button>
          <button v-if="openPlaylist" class="border rounded px-2 text-xs shrink-0" :title="`Add to ${openPlaylist.name}`" @click="addSong(song)">
            +
          </button>
          <img v-if="song.cover" :src="coverUrl(song.cover)" class="w-10 h-10 rounded object-cover shrink-0" alt="" />
          <div v-else class="w-10 h-10 rounded bg-muted shrink-0" />
          <div class="min-w-0">
            <div class="truncate">{{ song.title }}</div>
            <div class="truncate text-xs text-muted-foreground">
              {{ [song.artist, song.album].filter(Boolean).join(' — ') }}
              <span v-if="song.albumArtist && song.albumArtist !== song.artist">(album: {{ song.albumArtist }})</span>
              <span v-if="song.playCount">· played {{ song.playCount }}×</span>
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
          <button class="border rounded px-2 text-xs shrink-0" title="Play" @click="play(`album:${album.id}`)">▶</button>
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
      <div class="space-y-2 border-t pt-2">
        <div class="flex flex-wrap gap-2">
          <input v-model="playlistName" class="border rounded px-2 py-1 text-sm" placeholder="Playlist name" />
          <button class="border rounded px-3 py-1" @click="createPlaylist">Create playlist</button>
          <button class="border rounded px-3 py-1" @click="run('Playlists', showPlaylists)">Show playlists</button>
        </div>
        <div v-for="playlist in playlists ?? []" :key="playlist.id" class="flex flex-wrap items-center gap-2 text-sm">
          <div class="grid grid-cols-2 w-10 h-10 rounded overflow-hidden bg-muted shrink-0">
            <img v-for="cover in playlist.covers" :key="cover" :src="coverUrl(cover)" class="w-5 h-5 object-cover" alt="" />
          </div>
          <span>{{ playlist.name }}</span>
          <span class="text-xs text-muted-foreground">
            {{ plural(playlist.entries, 'song') }}{{ playlist.available < playlist.entries ? ` (${playlist.available} available)` : '' }}
            · {{ duration(playlist.durationMs) }}
            <template v-if="playlist.kind === 'linked'">
              · linked to {{ playlist.file }}{{ playlist.fileMissing ? ' (file gone)' : '' }}
              {{ playlist.suggested ? ` · ${playlist.suggested} suggested` : '' }}
            </template>
          </span>
          <button class="border rounded px-2 text-xs" @click="openList(playlist)">Open</button>
          <button class="border rounded px-2 text-xs" @click="renamePlaylist(playlist)">Rename to the name above</button>
          <button class="border rounded px-2 text-xs" @click="deletePlaylist(playlist)">Delete</button>
        </div>
        <div v-if="openPlaylist" class="space-y-1 border rounded p-2">
          <div class="text-sm font-medium">
            <button class="border rounded px-2 text-xs" title="Play" @click="play(`playlist:${openPlaylist.id}`)">▶</button>
            {{ openPlaylist.name }}
            <span class="text-xs text-muted-foreground">(open "Show songs" and use + to add songs)</span>
          </div>
          <div v-if="openPlaylist.kind === 'linked'" class="flex flex-wrap gap-2 text-xs">
            <span class="text-muted-foreground">{{ openPlaylist.file }}{{ openPlaylist.fileMissing ? ' (file gone)' : '' }}:</span>
            <button class="border rounded px-2" @click="toggleOption('newInFile')">New in file: {{ openPlaylist.newInFile }}</button>
            <button class="border rounded px-2" @click="toggleOption('removedFromFile')">
              Removed from file: {{ openPlaylist.removedFromFile }}
            </button>
            <button class="border rounded px-2" @click="toggleOption('writeBack')">Write back: {{ openPlaylist.writeBack ? 'on' : 'off' }}</button>
            <button class="border rounded px-2" @click="unlink">Unlink</button>
          </div>
          <div v-for="entry in entries" :key="entry.id" class="flex items-center gap-2 text-sm" :class="{ 'opacity-40': greyed(entry) }">
            <span class="text-xs text-muted-foreground w-5 text-right">{{ entry.position + 1 }}</span>
            <template v-if="entry.track">
              <span class="truncate">{{ entry.track.title }}</span>
              <span class="text-xs text-muted-foreground truncate">{{ entry.track.artist }}</span>
              <span v-if="!entry.track.available" class="text-xs">(unavailable)</span>
            </template>
            <span v-else class="truncate text-xs">not found: {{ entry.fileEntry }}</span>
            <span v-if="stateLabel[entry.state]" class="text-xs italic">{{ stateLabel[entry.state] }}</span>
            <button
              v-if="entry.state === 'removed_here' || entry.state === 'suggested'"
              class="border rounded px-2 text-xs"
              @click="restoreEntry(entry)"
            >
              {{ entry.state === 'suggested' ? 'Add' : 'Restore' }}
            </button>
            <button class="border rounded px-2 text-xs" @click="moveEntry(entry, -1)">↑</button>
            <button class="border rounded px-2 text-xs" @click="moveEntry(entry, 1)">↓</button>
            <button class="border rounded px-2 text-xs" @click="removeEntry(entry)">✕</button>
          </div>
        </div>
      </div>
      <div v-if="artists" class="space-y-1">
        <div class="text-xs text-muted-foreground">The first {{ artists.length }} artists by name</div>
        <div v-for="artist in artists" :key="artist.id" class="text-sm">
          <button class="border rounded px-2 text-xs" title="Play" @click="play(`artist:${artist.id}`)">▶</button>
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
