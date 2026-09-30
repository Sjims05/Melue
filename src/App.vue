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
 import { ref } from 'vue'
import { SmbSource } from './sources/SmbSource'
import {
  initialize,
  setSource,
  setQueue,
  next,
  previous,
  setShuffle,
  setRepeatMode,
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
    await setSource({
      src: localCopyPath,
      id: 1,
      title: 'Spike Test Track',
      artist: 'Spike Test',
    })
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
      items: testQueue.map((path, i) => ({ src: path, id: i + 1, title: `Track ${i + 1}`, artist: 'Spike Test' })),
      startIndex: 0,
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