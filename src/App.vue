<script setup lang="ts">
/**
 * SpikeTests.vue (v2)
 *
 * Rebuilt after switching from "browse SMB via a SAF file manager app"
 * to "connect to SMB directly via SmbSource". Spikes 3 and 4 (playback,
 * queue) now get their test files from an SMB download instead of a
 * SAF folder pick.
 *
 * android-fs is kept, but repurposed: its real future job is letting
 * the user pick *where on the device* to store the local library
 * (SD card vs internal storage), not reaching the SMB share. That's
 * section 2 below, and it's independent of everything else on this page.
 *
 * SMB credentials are read from .env (gitignored), see .env.example.
 */

import { appDataDir, join } from '@tauri-apps/api/path'
import { onMounted, ref } from 'vue'
import { SmbSource } from './sources/SmbSource'
import { readFile } from '@tauri-apps/plugin-fs'
import { parseBuffer } from 'music-metadata'
import {
  initialize,
  setSource,
  setQueue,
  next,
  previous,
  setShuffle,
  setRepeatMode,
  setLibrary,
  addToQueue,
  removeFromQueue,
  getQueue,
  restoreLastQueue,
  setOptions,
  setControls,
  onControlPressed,
  setTrackedLists,
  onTrackedListChanged,
  play,
  getState,
  addStateListener,
  type NativeAudioState,
  type NativeAudioRepeatMode,
} from 'tauri-plugin-native-audio-api'

// ---- Logging -------------------------------------------------------
const log = ref<string[]>([])
function logMsg(msg: string) {
  const line = `[${new Date().toLocaleTimeString()}] ${msg}`
  console.log(line)
  log.value = [line, ...log.value].slice(0, 100)
}

// ---- SMB settings come from .env (see .env.example) -----------------
const SMB_HOST = import.meta.env.VITE_SMB_HOST
const SMB_USER = import.meta.env.VITE_SMB_USER
const SMB_PASS = import.meta.env.VITE_SMB_PASS
const SMB_SHARE = import.meta.env.VITE_SMB_SHARE
const SMB_MUSIC_PATH = import.meta.env.VITE_SMB_MUSIC_PATH
// -----------------------------------------------------------------------

const source = new SmbSource()
let connected = false

async function ensureConnected() {
  if (connected) return
  await source.connect({ host: SMB_HOST, user: SMB_USER, pass: SMB_PASS, share: SMB_SHARE })
  connected = true
  logMsg('SMB: connected')
}

// ---- Song names from the files' own tags ---------------------------
type TrackInfo = { title: string; artist?: string; album?: string }
const trackInfoCache = new Map<string, TrackInfo>()

// Title, artist(s) and album from the file's tags; `fallbackTitle` when it has none.
async function readTrackInfo(path: string, fallbackTitle: string): Promise<TrackInfo> {
  const cached = trackInfoCache.get(path)
  if (cached) return cached
  let info: TrackInfo = { title: fallbackTitle }
  try {
    const { common } = await parseBuffer(await readFile(path), { mimeType: 'audio/mpeg' }, { skipCovers: true })
    const artists = common.artists?.length ? common.artists.join(', ') : common.artist
    info = { title: common.title || fallbackTitle, artist: artists || undefined, album: common.album || undefined }
  } catch (e) {
    logMsg(`Tags: couldn't read ${path}: ${e}`)
  }
  trackInfoCache.set(path, info)
  return info
}

// =====================================================================
// SECTION 1: SMB — connect, list, download one file
// =====================================================================
let localCopyPath: string | null = null

async function smb_listFiles() {
  try {
    await ensureConnected()
    const entries = await source.listFiles(SMB_MUSIC_PATH)
    logMsg(`SMB: found ${entries.length} entries`)
    for (const e of entries.slice(0, 20)) {
      logMsg(`  - [${e.isDirectory ? 'DIR' : 'FILE'}] ${e.name} (${e.size} bytes)`)
    }
  } catch (e) {
    logMsg(`SMB list FAILED: ${e}`)
  }
}

async function smb_downloadOneFile() {
  try {
    await ensureConnected()
    const entries = await source.listFiles(SMB_MUSIC_PATH)
    const mp3 = entries.find((e) => !e.isDirectory && e.name.toLowerCase().endsWith('.mp3'))
    if (!mp3) {
      logMsg('SMB: no mp3 found')
      return
    }

    const destPath = await join(await appDataDir(), 'spike-test.mp3')

    logMsg(`SMB: downloading ${mp3.name}...`)
    await source.copyFile(`${SMB_MUSIC_PATH}/${mp3.name}`, destPath)

    localCopyPath = destPath
    logMsg(`SMB: downloaded to ${destPath}`)
  } catch (e) {
    logMsg(`SMB download FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 2: (future feature demo) pick a local storage destination
// via android-fs. Independent of the SMB flow above — this is about
// where downloaded files eventually live, not about reaching the share.
// =====================================================================
async function pickLocalStorageFolder() {
  try {
    // VERIFY: confirm this against node_modules/tauri-plugin-android-fs-api
    // if it errors — API surface has shifted between versions before.
    const AndroidFs = await import('tauri-plugin-android-fs-api')
    const uri = await AndroidFs.showOpenDirPicker()
    if (!uri) {
      logMsg('Storage picker: cancelled')
      return
    }
    logMsg(`Storage picker: picked ${uri.uri} (not wired to anything yet — just proving the picker works)`)
  } catch (e) {
    logMsg(`Storage picker FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 3: play the local copy with native-audio
// =====================================================================
async function playLocalFile() {
  if (!localCopyPath) {
    logMsg('Play: download a file first (section 1)')
    return
  }
  try {

    await addStateListener((state: any) => {
      logMsg(`Playback state: ${JSON.stringify(state)}`)
    })

    await initialize()
    await setSource({ src: localCopyPath, id: 1, ...(await readTrackInfo(localCopyPath, 'Downloaded track')) })
    await play()
    const state = await getState()
    logMsg(`Play called, state = ${JSON.stringify(state)}`)
  } catch (e) {
    logMsg(`Play FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 4: queue of local files, lock the screen, see if it advances
// =====================================================================
const testQueue: string[] = []

async function downloadQueueTracks() {
  try {
    await ensureConnected()
    const entries = await source.listFiles(SMB_MUSIC_PATH)
    const mp3s = entries.filter((e) => !e.isDirectory && e.name.toLowerCase().endsWith('.mp3')).slice(0, 4)
    if (mp3s.length === 0) {
      logMsg('Queue: no mp3s found')
      return
    }

    testQueue.length = 0

    for (let i = 0; i < mp3s.length; i++) {
      const entry = mp3s[i]
      const destPath = await join(await appDataDir(), `spike-queue-${i}.mp3`)
      logMsg(`Queue: downloading ${entry.name}...`)
      await source.copyFile(`${SMB_MUSIC_PATH}/${entry.name}`, destPath)
      testQueue.push(destPath)
    }
    logMsg(`Queue: downloaded ${testQueue.length} tracks, ready to test`)
  } catch (e) {
    logMsg(`Queue download FAILED: ${e}`)
  }
}

// The whole queue lives in the native player, so it keeps advancing while the
// WebView is suspended, and the notification / lock screen prev & next buttons
// move between tracks. JS only listens for track changes to log them.
let queueListening = false
let lastQueueIndex = -1

async function listenToQueue() {
  if (queueListening) return
  queueListening = true
  await addStateListener((state: NativeAudioState) => {
    shuffleOn.value = state.shuffle
    repeatMode.value = state.repeatMode
    if (state.queueIndex !== lastQueueIndex) {
      lastQueueIndex = state.queueIndex
      if (state.queueIndex >= 0) {
        logMsg(`Queue: now on track ${state.queueIndex + 1}/${state.queueLength} (id ${state.currentId})`)
      }
    }
    if (state.status === 'ended') {
      logMsg('Queue finished — PASS if it advanced while the screen was locked')
    }
    if (state.status === 'error') {
      logMsg(`Queue error: ${state.error}`)
    }
  })
}

async function startQueue() {
  if (testQueue.length === 0) {
    logMsg('Queue: download tracks first')
    return
  }
  try {
    await listenToQueue()
    await initialize()
    await setQueue({
      // ids start at 1: the plugin ignores ids <= 0 for progress checkpoints
      items: await testTracks(),
      startIndex: 0,
      // Plays the "All songs" folder, so it counts towards recently played playlists.
      sourceId: 'songs',
    })
    await play()
    logMsg('Queue started. Now lock the screen and wait, or use the notification prev/next buttons.')
  } catch (e) {
    logMsg(`Queue start FAILED: ${e}`)
  }
}

async function queuePrevious() {
  try {
    await previous()
  } catch (e) {
    logMsg(`Queue previous FAILED: ${e}`)
  }
}

async function queueNext() {
  try {
    await next()
  } catch (e) {
    logMsg(`Queue next FAILED: ${e}`)
  }
}

// Shuffle / repeat live in the native player and survive setQueue, so they can
// be toggled before or after starting the queue.
const shuffleOn = ref(false)
const repeatMode = ref<NativeAudioRepeatMode>('off')
const nextRepeatMode: Record<NativeAudioRepeatMode, NativeAudioRepeatMode> = { off: 'all', all: 'one', one: 'off' }

async function toggleShuffle() {
  try {
    const state = await setShuffle(!shuffleOn.value)
    shuffleOn.value = state.shuffle
    logMsg(`Shuffle: ${state.shuffle ? 'on' : 'off'}`)
  } catch (e) {
    logMsg(`Shuffle FAILED: ${e}`)
  }
}

async function cycleRepeatMode() {
  try {
    const state = await setRepeatMode(nextRepeatMode[repeatMode.value])
    repeatMode.value = state.repeatMode
    logMsg(`Repeat: ${state.repeatMode}`)
  } catch (e) {
    logMsg(`Repeat FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 4b: Android Auto library (carSupport in tauri.conf.json)
// Sent once, then saved natively: Auto can browse and play it with the app closed.
// =====================================================================
// The section 4 downloads; their files stay in appData between launches.
async function testTracks() {
  const paths = await Promise.all([0, 1, 2, 3].map(async (i) => join(await appDataDir(), `spike-queue-${i}.mp3`)))
  return Promise.all(paths.map(async (src, i) => ({ src, id: i + 1, ...(await readTrackInfo(src, `Track ${i + 1}`)) })))
}

// The playable folders of the test library, by id.
const folderTitles: Record<string, string> = { songs: 'All songs', 'pl-test': 'Test playlist' }

// "Recently played playlists", kept by the plugin itself: it records which playable folder a queue
// was started from (picked in Android Auto, or setQueue's sourceId), also with the app closed.
async function setUpRecentPlaylists() {
  await setTrackedLists([{ id: 'recentPlaylists', track: 'folder', limit: 5 }])
  // Changes from while the app was closed arrive first. The real app would save them in its database.
  await onTrackedListChanged(({ listId, ids }) => {
    const names = ids.map((id) => folderTitles[String(id)] ?? String(id))
    logMsg(`Tracked list "${listId}": ${names.join(', ') || '(empty)'}`)
  })
}

async function sendTestLibrary() {
  try {
    // Headers by first letter in Android Auto's lists.
    const items = (await testTracks()).map((t) => ({ ...t, group: t.title.charAt(0).toUpperCase() }))
    await setLibrary({
      items,
      emptyMessage: 'Open phoenix to sync your music',
      root: [
        // Fills itself from the tracked list; nothing to rebuild when it changes.
        { id: 'recent', title: 'Recently played', style: 'list', trackedList: 'recentPlaylists' },
        { id: 'songs', title: 'All songs', playable: true, style: 'list', children: [1, 2, 3, 4] },
        {
          id: 'playlists',
          title: 'Playlists',
          style: 'list',
          children: [{ id: 'pl-test', title: 'Test playlist', subtitle: '3rd and 1st downloaded song', playable: true, children: [3, 1] }],
        },
      ],
    })
    logMsg('Library sent to Android Auto: Recently played + All songs (4) + Playlists > Test playlist (2)')
  } catch (e) {
    logMsg(`setLibrary FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 4c: queue editing + resuming the last queue
// =====================================================================
async function logQueue() {
  try {
    const { items, currentIndex, playOrder } = await getQueue()
    const upNext = playOrder.map((i) => `${items[i].title}${i === currentIndex ? ' (playing)' : ''}`)
    logMsg(`Queue in play order: ${upNext.join(' → ') || '(empty)'}`)
  } catch (e) {
    logMsg(`getQueue FAILED: ${e}`)
  }
}

async function playTrack1Next() {
  try {
    await addToQueue([(await testTracks())[0]], { playNext: true })
    await logQueue()
  } catch (e) {
    logMsg(`addToQueue FAILED: ${e}`)
  }
}

async function addTrack4ToEnd() {
  try {
    await addToQueue([(await testTracks())[3]])
    await logQueue()
  } catch (e) {
    logMsg(`addToQueue FAILED: ${e}`)
  }
}

async function removeUpNext() {
  try {
    const { currentIndex, playOrder } = await getQueue()
    const upNext = playOrder[playOrder.indexOf(currentIndex) + 1]
    if (upNext === undefined) {
      logMsg('Queue: nothing up next')
      return
    }
    await removeFromQueue(upNext)
    await logQueue()
  } catch (e) {
    logMsg(`removeFromQueue FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 4d: options + car buttons
// =====================================================================
// The plugin has no getter for options, so the test page remembers what it last set.
function savedFlag(key: string, fallback: boolean) {
  const saved = localStorage.getItem(key)
  return saved === null ? fallback : saved === 'true'
}
// Same defaults as the plugin.
const resumeOn = ref(savedFlag('resumeLastQueue', true))
const repeatAddedOn = ref(savedFlag('repeatAddedTracks', false))

async function toggleResume() {
  resumeOn.value = !resumeOn.value
  localStorage.setItem('resumeLastQueue', String(resumeOn.value))
  await setOptions({ resumeLastQueue: resumeOn.value })
  logMsg(`Auto-resume: ${resumeOn.value ? 'on' : 'off'}`)
}

async function toggleRepeatAdded() {
  repeatAddedOn.value = !repeatAddedOn.value
  localStorage.setItem('repeatAddedTracks', String(repeatAddedOn.value))
  await setOptions({ repeatAddedTracks: repeatAddedOn.value })
  logMsg(`Repeat added tracks: ${repeatAddedOn.value ? 'on' : 'off'}`)
}

async function setUpCarButtons() {
  await setControls({
    buttons: [
      { action: 'seekBack', seconds: 15 },
      { action: 'seekForward', seconds: 30 },
      { action: 'shuffle' },
      { action: 'repeat' },
      { action: 'speed', rates: [1, 1.5, 2] },
      { action: 'custom', id: 'like', label: 'Like', icon: 'heart', toggle: true },
    ],
  })
  // Presses made in the car while the app was closed arrive here first, then live ones.
  await onControlPressed((press) => {
    const time = new Date(press.pressedAtMs).toLocaleTimeString()
    logMsg(`Button "${press.buttonId}" on track id ${press.itemId}: ${press.active === null ? 'pressed' : press.active ? 'on' : 'off'} (at ${time})`)
  })
}

// Like the real app would: continue the last queue at startup (paused).
onMounted(async () => {
  try {
    await listenToQueue()
    await initialize()
    await setOptions({ resumeLastQueue: resumeOn.value, repeatAddedTracks: repeatAddedOn.value })
    await setUpCarButtons()
    await setUpRecentPlaylists()
    const state = await restoreLastQueue()
    if (state) {
      logMsg(`Restored last queue: track ${state.queueIndex + 1}/${state.queueLength} at ${Math.round(state.currentTime)}s, shuffle ${state.shuffle ? 'on' : 'off'}, repeat ${state.repeatMode}`)
      await logQueue()
    } else {
      logMsg('No saved queue to restore')
    }
  } catch (e) {
    logMsg(`Restore FAILED: ${e}`)
  }
})

// =====================================================================
// SECTION 5: Screen Wake Lock — request / release
// =====================================================================
let wakeLockSentinel: any = null

async function requestWakeLock() {
  try {
    if (!('wakeLock' in navigator)) {
      logMsg('Wake lock: navigator.wakeLock not available in this WebView')
      return
    }
    wakeLockSentinel = await (navigator as any).wakeLock.request('screen')
    logMsg('Wake lock acquired — leave the phone alone and see if the screen stays on')
    wakeLockSentinel.addEventListener('release', () => {
      logMsg('Wake lock was released (by OS or by us)')
    })
  } catch (e) {
    logMsg(`Wake lock FAILED: ${e}`)
  }
}

async function releaseWakeLock() {
  try {
    if (wakeLockSentinel) {
      await wakeLockSentinel.release()
      wakeLockSentinel = null
      logMsg('Wake lock released manually')
    } else {
      logMsg('No active wake lock to release')
    }
  } catch (e) {
    logMsg(`Wake lock release FAILED: ${e}`)
  }
}

document.addEventListener('visibilitychange', async () => {
  if (wakeLockSentinel !== null && document.visibilityState === 'visible') {
    await requestWakeLock()
  }
})

// =====================================================================
// SECTION 6: SQLite write/read
// =====================================================================
async function sqliteTest() {
  try {
    const Database = (await import('@tauri-apps/plugin-sql')).default
    const db = await Database.load('sqlite:spike-test.db')
    await db.execute('CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY, name TEXT)')
    await db.execute('INSERT INTO test (name) VALUES ($1)', ['hello from spike'])
    const rows = await db.select<{ id: number; name: string }[]>('SELECT * FROM test')
    logMsg(`SQLite: rows = ${JSON.stringify(rows)}`)
  } catch (e) {
    logMsg(`SQLite FAILED: ${e}`)
  }
}
</script>

<template>
  <div class="p-4 space-y-6 max-w-xl mx-auto">
    <h1 class="text-xl font-bold">Spike Tests</h1>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">1. SMB — connect, list, download</h2>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="smb_listFiles">List files</button>
        <button class="border rounded px-3 py-1" @click="smb_downloadOneFile">Download one file</button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">2. Local storage picker (future feature, not wired up)</h2>
      <button class="border rounded px-3 py-1" @click="pickLocalStorageFolder">Pick storage folder</button>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">3. Play local file (native-audio)</h2>
      <p class="text-sm text-muted-foreground">Run section 1's download first.</p>
      <button class="border rounded px-3 py-1" @click="playLocalFile">Play downloaded file</button>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4. Queue + lock screen test</h2>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="downloadQueueTracks">Download 4 tracks</button>
        <button class="border rounded px-3 py-1" @click="startQueue">Start queue</button>
      </div>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="queuePrevious">Previous</button>
        <button class="border rounded px-3 py-1" @click="queueNext">Next</button>
      </div>
      <div class="flex gap-2">
        <button
          class="border rounded px-3 py-1"
          :class="{ 'bg-primary text-primary-foreground': shuffleOn }"
          @click="toggleShuffle"
        >
          Shuffle: {{ shuffleOn ? 'on' : 'off' }}
        </button>
        <button
          class="border rounded px-3 py-1"
          :class="{ 'bg-primary text-primary-foreground': repeatMode !== 'off' }"
          @click="cycleRepeatMode"
        >
          Repeat: {{ repeatMode }}
        </button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4b. Android Auto library</h2>
      <p class="text-sm text-muted-foreground">Uses the 4 tracks from section 4 (download them once).</p>
      <button class="border rounded px-3 py-1" @click="sendTestLibrary()">Send test library</button>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4c. Queue editing</h2>
      <p class="text-sm text-muted-foreground">The last queue is restored when the app opens.</p>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="logQueue">Show queue</button>
        <button class="border rounded px-3 py-1" @click="playTrack1Next">Play 1st song next</button>
        <button class="border rounded px-3 py-1" @click="addTrack4ToEnd">Add 4th song to end</button>
        <button class="border rounded px-3 py-1" @click="removeUpNext">Remove up next</button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4d. Options</h2>
      <p class="text-sm text-muted-foreground">Car buttons are set up when the app opens; presses show in the log.</p>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="toggleResume">Auto-resume: {{ resumeOn ? 'on' : 'off' }}</button>
        <button class="border rounded px-3 py-1" @click="toggleRepeatAdded">
          Repeat added tracks: {{ repeatAddedOn ? 'on' : 'off' }}
        </button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">5. Screen wake lock</h2>
      <div class="flex gap-2">
        <button class="border rounded px-3 py-1" @click="requestWakeLock">Request wake lock</button>
        <button class="border rounded px-3 py-1" @click="releaseWakeLock">Release wake lock</button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">6. SQLite write/read</h2>
      <button class="border rounded px-3 py-1" @click="sqliteTest">Run SQLite test</button>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">Log</h2>
      <div class="text-xs font-mono max-h-96 overflow-y-auto space-y-1">
        <div v-for="(line, i) in log" :key="i">{{ line }}</div>
      </div>
    </section>
  </div>
</template>