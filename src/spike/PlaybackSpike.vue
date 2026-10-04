<script setup lang="ts">
/**
 * Spike sections for playback and the queue, on every platform: Android / iOS play through the
 * native-audio plugin's native player, desktop through its Rust player. Goes through the platform Player.
 */
import { appDataDir, join } from '@tauri-apps/api/path'
import { onMounted, onUnmounted, ref } from 'vue'
import { player, type PlayerState, type RepeatMode, type UpdateQueueOptions } from '@/platform'
import { SMB_MUSIC_PATH, ensureConnected, localCopyPath, logMsg, readTrackInfo, source, testTracks } from './spike'

// =====================================================================
// SECTION 3: play the local copy
// =====================================================================
async function playLocalFile() {
  if (!localCopyPath.value) {
    logMsg('Play: download a file first (section 1)')
    return
  }
  try {
    const p = player
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
// SECTION 4: queue of local files (on a phone: lock the screen, see if it advances)
// =====================================================================
const downloaded = ref(0)

async function downloadQueueTracks() {
  try {
    await ensureConnected()
    const entries = await source.listFiles(SMB_MUSIC_PATH)
    const mp3s = entries.filter((e) => !e.isDirectory && e.name.toLowerCase().endsWith('.mp3')).slice(0, 4)
    if (mp3s.length === 0) {
      logMsg('Queue: no mp3s found')
      return
    }
    for (let i = 0; i < mp3s.length; i++) {
      const entry = mp3s[i]
      const destPath = await join(await appDataDir(), `spike-queue-${i}.mp3`)
      logMsg(`Queue: downloading ${entry.name}...`)
      await source.copyFile(`${SMB_MUSIC_PATH}/${entry.name}`, destPath)
    }
    downloaded.value = mp3s.length
    logMsg(`Queue: downloaded ${mp3s.length} tracks, ready to test`)
  } catch (e) {
    logMsg(`Queue download FAILED: ${e}`)
  }
}

// The whole queue lives in the player (native on phones, Rust on desktop), so it keeps advancing
// on its own. JS only listens for track changes to log them.
let queueListening = false
let lastQueueIndex = -1
const playing = ref(false)
const position = ref('')
// The sleep timer as the state reports it: the time it pauses, or the end of the track.
const sleepEndsAt = ref<number | null>(null)
const sleepEndOfTrack = ref(false)
const now = ref(Date.now())
const clock = setInterval(() => (now.value = Date.now()), 1000)
onUnmounted(() => clearInterval(clock))
const sleepText = () =>
  sleepEndOfTrack.value
    ? 'pauses at the end of this song'
    : sleepEndsAt.value
      ? `pauses in ${Math.max(0, Math.round((sleepEndsAt.value - now.value) / 1000))} s`
      : 'off'

async function listenToQueue() {
  if (queueListening) return
  queueListening = true
  await player.onState((state: PlayerState) => {
    shuffleOn.value = state.shuffle
    repeatMode.value = state.repeatMode
    playing.value = state.isPlaying
    const time = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`
    position.value = state.queueIndex >= 0 ? `${time(state.currentTime)} / ${time(state.duration)}` : ''
    sleepEndsAt.value = state.sleepTimerEndsAtMs
    sleepEndOfTrack.value = state.sleepTimerEndOfTrack
    if (state.queueIndex !== lastQueueIndex) {
      lastQueueIndex = state.queueIndex
      if (state.queueIndex >= 0) {
        logMsg(`Queue: now on track ${state.queueIndex + 1}/${state.queueLength} (id ${state.currentId})`)
      }
    }
    if (state.status === 'ended') {
      logMsg('Queue finished')
    }
    if (state.status === 'error' || state.error) {
      logMsg(`Queue error: ${state.error}`)
    }
  })
}

async function startQueue() {
  try {
    const p = player
    await listenToQueue()
    await p.initialize()
    // ids start at 1: the plugin ignores ids <= 0 for progress checkpoints.
    // Plays the "All songs" folder, so it counts towards recently played playlists.
    await p.setQueue(await testTracks(), { startIndex: 0, sourceId: 'songs' })
    await p.play()
    logMsg('Queue started.')
  } catch (e) {
    logMsg(`Queue start FAILED: ${e}`)
  }
}

async function run(label: string, action: () => Promise<unknown>) {
  try {
    await action()
  } catch (e) {
    logMsg(`${label} FAILED: ${e}`)
  }
}

const queuePrevious = () => run('Previous', async () => player.previous())
const queueNext = () => run('Next', async () => player.next())
const togglePlay = () => run('Play / pause', async () => (playing.value ? player.pause() : player.play()))
const sleepInOneMinute = () => run('Sleep timer', async () => player.setSleepTimer({ minutes: 1, fadeOutSeconds: 10 }))
const sleepAtEndOfSong = () => run('Sleep timer', async () => player.setSleepTimer({ endOfTrack: true }))
const cancelSleep = () => run('Sleep timer', async () => player.cancelSleepTimer())

const forward30 = () =>
  run('Seek', async () => {
    const p = player
    await p.seekTo((await p.getState()).currentTime + 30)
  })

// Shuffle / repeat live in the player and survive setQueue, so they can be toggled before or
// after starting the queue.
const shuffleOn = ref(false)
const repeatMode = ref<RepeatMode>('off')
const nextRepeatMode: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' }

async function toggleShuffle() {
  try {
    const state = await player.setShuffle(!shuffleOn.value)
    shuffleOn.value = state.shuffle
    logMsg(`Shuffle: ${state.shuffle ? 'on' : 'off'}`)
  } catch (e) {
    logMsg(`Shuffle FAILED: ${e}`)
  }
}

async function cycleRepeatMode() {
  try {
    const state = await player.setRepeatMode(nextRepeatMode[repeatMode.value])
    repeatMode.value = state.repeatMode
    logMsg(`Repeat: ${state.repeatMode}`)
  } catch (e) {
    logMsg(`Repeat FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 4c: queue editing + resuming the last queue
// =====================================================================
async function logQueue() {
  try {
    const { items, currentIndex, playOrder } = await player.getQueue()
    const name = (i: number) => `${items[i].title}${i === currentIndex ? ' (playing)' : ''}`
    const inPlayOrder = playOrder.map(name).join(' → ') || '(empty)'
    const inList = items.map((_, i) => name(i)).join(', ')
    // Shuffled, the two differ: the list is the playlist's order, the play order what plays when.
    logMsg(inList === playOrder.map(name).join(', ') ? `Queue: ${inPlayOrder}` : `Queue list: ${inList} | play order: ${inPlayOrder}`)
  } catch (e) {
    logMsg(`getQueue FAILED: ${e}`)
  }
}

async function playTrack1Next() {
  try {
    await player.addToQueue([(await testTracks())[0]], { playNext: true })
    await logQueue()
  } catch (e) {
    logMsg(`addToQueue FAILED: ${e}`)
  }
}

async function addTrack4ToEnd() {
  try {
    await player.addToQueue([(await testTracks())[3]])
    await logQueue()
  } catch (e) {
    logMsg(`addToQueue FAILED: ${e}`)
  }
}

// As if a sync changed the "All songs" playlist: with the 2nd song in the queue, it's removed and the
// 4th moved to the front; without it, the 2nd song comes back (as a new song, at the end of the playlist).
const updateChoices = {
  removedCurrent: ['finish', 'skip'],
  removedItems: ['remove', 'keep'],
  newItems: ['inPlace', 'end', 'next'],
  order: ['playlist', 'queue'],
} as const
const updateOptions = ref<Required<Omit<UpdateQueueOptions, 'sourceId'>>>({
  removedCurrent: 'finish',
  removedItems: 'remove',
  newItems: 'inPlace',
  order: 'playlist',
})

function cycleUpdateOption(key: keyof typeof updateChoices) {
  const choices = updateChoices[key] as readonly string[]
  const next = choices[(choices.indexOf(updateOptions.value[key]) + 1) % choices.length]
  updateOptions.value = { ...updateOptions.value, [key]: next }
}

async function simulatePlaylistChange() {
  try {
    const p = player
    const [first, second, third, fourth] = await testTracks()
    const secondQueued = (await p.getQueue()).items.some((item) => item.id === second.id)
    const items = secondQueued ? [fourth, first, third] : [fourth, first, third, second]
    logMsg(`New playlist: ${items.map((t) => t.title).join(', ')}`)
    const { added, removed } = await p.updateQueue(items, { sourceId: 'songs', ...updateOptions.value })
    logMsg(`Queue updated: ${added} added, ${removed} removed`)
    await logQueue()
  } catch (e) {
    logMsg(`updateQueue FAILED: ${e}`)
  }
}

async function removeUpNext() {
  try {
    const p = player
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

// Like the real app would: continue the last queue at startup (paused).
onMounted(async () => {
  try {
    const p = player
    await listenToQueue()
    await p.initialize()
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
  <section class="space-y-2 border rounded p-3">
    <h2 class="font-semibold">3. Play local file</h2>
    <p class="text-sm text-muted-foreground">Run section 1's download first.</p>
    <button class="border rounded px-3 py-1" @click="playLocalFile">Play downloaded file</button>
  </section>

  <section class="space-y-2 border rounded p-3">
    <h2 class="font-semibold">4. Queue</h2>
    <p class="text-sm text-muted-foreground">
      Download once; the files stay between launches. {{ downloaded ? `${downloaded} downloaded.` : '' }}
    </p>
    <div class="flex gap-2">
      <button class="border rounded px-3 py-1" @click="downloadQueueTracks">Download 4 tracks</button>
      <button class="border rounded px-3 py-1" @click="startQueue">Start queue</button>
    </div>
    <div class="flex flex-wrap gap-2 items-center">
      <button class="border rounded px-3 py-1" @click="queuePrevious">Previous</button>
      <button class="border rounded px-3 py-1" @click="togglePlay">{{ playing ? 'Pause' : 'Play' }}</button>
      <button class="border rounded px-3 py-1" @click="queueNext">Next</button>
      <button class="border rounded px-3 py-1" @click="forward30">+30 s</button>
      <span class="text-sm font-mono">{{ position }}</span>
    </div>
    <div class="flex flex-wrap gap-2 items-center">
      <button class="border rounded px-3 py-1" @click="sleepInOneMinute">Sleep in 1 min</button>
      <button class="border rounded px-3 py-1" @click="sleepAtEndOfSong">Sleep at end of song</button>
      <button class="border rounded px-3 py-1" @click="cancelSleep">Cancel sleep</button>
      <span class="text-sm">Sleep timer: {{ sleepText() }}</span>
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
    <h2 class="font-semibold">4c. Queue editing</h2>
    <p class="text-sm text-muted-foreground">The last queue is restored when the app opens.</p>
    <div class="flex flex-wrap gap-2">
      <button class="border rounded px-3 py-1" @click="logQueue">Show queue</button>
      <button class="border rounded px-3 py-1" @click="playTrack1Next">Play 1st song next</button>
      <button class="border rounded px-3 py-1" @click="addTrack4ToEnd">Add 4th song to end</button>
      <button class="border rounded px-3 py-1" @click="removeUpNext">Remove up next</button>
      <button class="border rounded px-3 py-1" @click="simulatePlaylistChange">Simulate playlist change</button>
      <!-- Options that another setting makes irrelevant are greyed out (still clickable, to set them up front). -->
      <button
        class="border rounded px-3 py-1"
        :class="{ 'opacity-40': updateOptions.removedItems === 'keep' }"
        :title="updateOptions.removedItems === 'keep' ? 'No effect: removed songs are kept, the playing one too' : ''"
        @click="cycleUpdateOption('removedCurrent')"
      >
        Removed playing song: {{ updateOptions.removedCurrent }}
      </button>
      <button class="border rounded px-3 py-1" @click="cycleUpdateOption('removedItems')">
        Removed songs: {{ updateOptions.removedItems }}
      </button>
      <button class="border rounded px-3 py-1" @click="cycleUpdateOption('newItems')">New songs: {{ updateOptions.newItems }}</button>
      <button
        class="border rounded px-3 py-1"
        :class="{ 'opacity-40': shuffleOn }"
        :title="shuffleOn ? 'No effect on what plays while shuffled: the play order is kept (only the list order changes)' : ''"
        @click="cycleUpdateOption('order')"
      >
        Order: {{ updateOptions.order }}
      </button>
    </div>
  </section>
</template>
