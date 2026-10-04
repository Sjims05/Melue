<script setup lang="ts">
/**
 * Spike tests for Android / iOS: the shared sections (SpikePage, PlaybackSpike) plus the plugin's
 * mobile-only features (Android Auto library, car buttons, options), which use the plugin directly.
 */
import { onMounted, ref } from 'vue'
import { setLibrary, setOptions, setControls, onControlPressed } from 'tauri-plugin-native-audio-api'
import { trackedLists } from '@/platform'
import PlaybackSpike from './PlaybackSpike.vue'
import SkipRulesOptions from './SkipRulesOptions.vue'
import SpikePage from './SpikePage.vue'
import { logMsg, testTracks } from './spike'

// =====================================================================
// SECTION 4b: Android Auto library (carSupport in tauri.conf.json)
// Sent once, then saved natively: Auto can browse and play it with the app closed.
// =====================================================================
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
// SECTION 4d: options + car buttons
// =====================================================================
// The plugin has no getter for options, so the test page remembers what it last set.
function savedFlag(key: string, fallback: boolean) {
  const saved = localStorage.getItem(key)
  return saved === null ? fallback : saved === 'true'
}
// Same defaults as the plugin.
// Auto-resume: off, paused (load the last queue) or play (load and play it). Older saves were true / false.
const resumeModes = ['paused', 'play', 'off'] as const
type ResumeMode = (typeof resumeModes)[number]
const savedResume = localStorage.getItem('resumeLastQueue')
const resumeMode = ref<ResumeMode>(
  savedResume === 'false' ? 'off' : resumeModes.includes(savedResume as ResumeMode) ? (savedResume as ResumeMode) : 'paused',
)
const repeatAddedOn = ref(savedFlag('repeatAddedTracks', false))
// How long the controls stay after a pause: with no limit while Android Auto is connected (if on),
// otherwise pausedKeepAliveMinutes. Same defaults as the plugin.
const carKeepAliveOn = ref(savedFlag('keepAliveWhileCarConnected', true))
const keepAliveChoices = [30, 5, 1, 0]
const savedMinutes = Number(localStorage.getItem('pausedKeepAliveMinutes'))
const keepAliveMinutes = ref(keepAliveChoices.includes(savedMinutes) && localStorage.getItem('pausedKeepAliveMinutes') !== null ? savedMinutes : 30)

async function cycleResume() {
  resumeMode.value = resumeModes[(resumeModes.indexOf(resumeMode.value) + 1) % resumeModes.length]
  localStorage.setItem('resumeLastQueue', resumeMode.value)
  await setOptions({ resumeLastQueue: resumeMode.value })
  logMsg(`Auto-resume: ${resumeMode.value}`)
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

onMounted(async () => {
  try {
    await setOptions({
      resumeLastQueue: resumeMode.value,
      repeatAddedTracks: repeatAddedOn.value,
      keepAliveWhileCarConnected: carKeepAliveOn.value,
      pausedKeepAliveMinutes: keepAliveMinutes.value,
    })
    await setUpCarButtons()
    await setUpRecentPlaylists()
  } catch (e) {
    logMsg(`Setup FAILED: ${e}`)
  }
})
</script>

<template>
  <SpikePage title="Spike Tests">
    <PlaybackSpike />

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4b. Android Auto library</h2>
      <p class="text-sm text-muted-foreground">Uses the 4 tracks from section 4 (download them once).</p>
      <button class="border rounded px-3 py-1" @click="sendTestLibrary()">Send test library</button>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4d. Options</h2>
      <p class="text-sm text-muted-foreground">Car buttons are set up when the app opens; presses show in the log.</p>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="cycleResume">Auto-resume: {{ resumeMode }}</button>
        <button class="border rounded px-3 py-1" @click="toggleRepeatAdded">
          Repeat added tracks: {{ repeatAddedOn ? 'on' : 'off' }}
        </button>
        <button class="border rounded px-3 py-1" @click="toggleCarKeepAlive">
          Keep controls in car: {{ carKeepAliveOn ? 'on' : 'off' }}
        </button>
        <button class="border rounded px-3 py-1" @click="cycleKeepAliveMinutes">
          After pause: {{ keepAliveMinutes }} min
        </button>
        <SkipRulesOptions />
      </div>
    </section>
  </SpikePage>
</template>
