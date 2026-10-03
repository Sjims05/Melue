<script setup lang="ts">
/**
 * Spike tests for Android / iOS: the shared sections (SpikePage) plus playback, the queue
 * and the plugin's other features (Android Auto library, buttons, tracked lists, options).
 *
 * Playback goes through the platform Player; the other features use the native-audio plugin directly for now.
 */
import { appDataDir, join } from '@tauri-apps/api/path'
import { onMounted, ref } from 'vue'
import {
  setLibrary,
  setOptions,
  setControls,
  onControlPressed,
} from 'tauri-plugin-native-audio-api'
import { player as platformPlayer, trackedLists, type PlayerState, type RepeatMode } from '@/platform'
import SpikePage from './SpikePage.vue'
import { SMB_MUSIC_PATH, ensureConnected, localCopyPath, logMsg, readTrackInfo, source } from './spike'

// The mobile build always has a player.
const player = async () => platformPlayer!

// =====================================================================
// SECTION 3: play the local copy
// =====================================================================
async function playLocalFile() {
  if (!localCopyPath.value) {
    logMsg('Play: download a file first (section 1)')
    return
  }
  try {
    const p = await player()
    await p.onState((state) => {
      logMsg(`Playback state: ${JSON.stringify(state)}`)
    })

    await p.initialize()
    await p.setQueue([{ src: localCopyPath.value, id: 1, ...(await readTrackInfo(localCopyPath.value, 'Downloaded track')) }])
    await p.play()
    const state = await p.getState()
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
  await (await player()).onState((state: PlayerState) => {
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
    const p = await player()
    await listenToQueue()
    await p.initialize()
    // ids start at 1: the plugin ignores ids <= 0 for progress checkpoints.
    // Plays the "All songs" folder, so it counts towards recently played playlists.
    await p.setQueue(await testTracks(), { startIndex: 0, sourceId: 'songs' })
    await p.play()
    logMsg('Queue started. Now lock the screen and wait, or use the notification prev/next buttons.')
  } catch (e) {
    logMsg(`Queue start FAILED: ${e}`)
  }
}

async function queuePrevious() {
  try {
    await (await player()).previous()
  } catch (e) {
    logMsg(`Queue previous FAILED: ${e}`)
  }
}

async function queueNext() {
  try {
    await (await player()).next()
  } catch (e) {
    logMsg(`Queue next FAILED: ${e}`)
  }
}

// Shuffle / repeat live in the native player and survive setQueue, so they can
// be toggled before or after starting the queue.
const shuffleOn = ref(false)
const repeatMode = ref<RepeatMode>('off')
const nextRepeatMode: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' }

async function toggleShuffle() {
  try {
    const state = await (await player()).setShuffle(!shuffleOn.value)
    shuffleOn.value = state.shuffle
    logMsg(`Shuffle: ${state.shuffle ? 'on' : 'off'}`)
  } catch (e) {
    logMsg(`Shuffle FAILED: ${e}`)
  }
}

async function cycleRepeatMode() {
  try {
    const state = await (await player()).setRepeatMode(nextRepeatMode[repeatMode.value])
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
  // The mobile build always has tracked lists.
  const lists = trackedLists!
  await lists.configure([{ id: 'recentPlaylists', track: 'folder', limit: 5 }])
  // Changes from while the app was closed arrive first. The real app would save them in its database.
  await lists.onChange(({ listId, entries }) => {
    const names = entries.map(({ id }) => folderTitles[String(id)] ?? String(id))
    logMsg(`Tracked list "${listId}": ${names.join(', ') || '(empty)'}`)
  })
}

async function sendTestLibrary() {
  try {
    // Headers by first letter in Android Auto's lists.
    const items = (await testTracks()).map((t) => ({ ...t, group: t.title.charAt(0).toUpperCase() }))
    await setLibrary({
      items,
      emptyMessage: 'Open Melue to sync your music',
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
    const { items, currentIndex, playOrder } = await (await player()).getQueue()
    const upNext = playOrder.map((i) => `${items[i].title}${i === currentIndex ? ' (playing)' : ''}`)
    logMsg(`Queue in play order: ${upNext.join(' → ') || '(empty)'}`)
  } catch (e) {
    logMsg(`getQueue FAILED: ${e}`)
  }
}

async function playTrack1Next() {
  try {
    await (await player()).addToQueue([(await testTracks())[0]], { playNext: true })
    await logQueue()
  } catch (e) {
    logMsg(`addToQueue FAILED: ${e}`)
  }
}

async function addTrack4ToEnd() {
  try {
    await (await player()).addToQueue([(await testTracks())[3]])
    await logQueue()
  } catch (e) {
    logMsg(`addToQueue FAILED: ${e}`)
  }
}

async function removeUpNext() {
  try {
    const p = await player()
    const { currentIndex, playOrder } = await p.getQueue()
    const upNext = playOrder[playOrder.indexOf(currentIndex) + 1]
    if (upNext === undefined) {
      logMsg('Queue: nothing up next')
      return
    }
    await p.removeFromQueue(upNext)
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
// How long the controls stay after a pause: with no limit while Android Auto is connected (if on),
// otherwise pausedKeepAliveMinutes. Same defaults as the plugin.
const carKeepAliveOn = ref(savedFlag('keepAliveWhileCarConnected', true))
const keepAliveChoices = [30, 5, 1, 0]
const savedMinutes = Number(localStorage.getItem('pausedKeepAliveMinutes'))
const keepAliveMinutes = ref(keepAliveChoices.includes(savedMinutes) && localStorage.getItem('pausedKeepAliveMinutes') !== null ? savedMinutes : 30)

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

async function toggleCarKeepAlive() {
  carKeepAliveOn.value = !carKeepAliveOn.value
  localStorage.setItem('keepAliveWhileCarConnected', String(carKeepAliveOn.value))
  await setOptions({ keepAliveWhileCarConnected: carKeepAliveOn.value })
  logMsg(`Keep controls while in the car: ${carKeepAliveOn.value ? 'on' : 'off'}`)
}

async function cycleKeepAliveMinutes() {
  keepAliveMinutes.value = keepAliveChoices[(keepAliveChoices.indexOf(keepAliveMinutes.value) + 1) % keepAliveChoices.length]
  localStorage.setItem('pausedKeepAliveMinutes', String(keepAliveMinutes.value))
  await setOptions({ pausedKeepAliveMinutes: keepAliveMinutes.value })
  logMsg(`Controls stay after a pause for: ${keepAliveMinutes.value} min`)
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
    const p = await player()
    await listenToQueue()
    await p.initialize()
    await setOptions({
      resumeLastQueue: resumeOn.value,
      repeatAddedTracks: repeatAddedOn.value,
      keepAliveWhileCarConnected: carKeepAliveOn.value,
      pausedKeepAliveMinutes: keepAliveMinutes.value,
    })
    await setUpCarButtons()
    await setUpRecentPlaylists()
    const state = await p.restoreLastQueue()
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
</script>

<template>
  <SpikePage title="Spike Tests">
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
        <button class="border rounded px-3 py-1" @click="toggleCarKeepAlive">
          Keep controls in car: {{ carKeepAliveOn ? 'on' : 'off' }}
        </button>
        <button class="border rounded px-3 py-1" @click="cycleKeepAliveMinutes">
          After pause: {{ keepAliveMinutes }} min
        </button>
      </div>
    </section>
  </SpikePage>
</template>
