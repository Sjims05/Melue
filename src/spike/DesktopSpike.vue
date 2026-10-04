<script setup lang="ts">
/**
 * Spike tests for Windows / macOS / Linux: the shared sections (SpikePage) and playback (PlaybackSpike),
 * through the native-audio plugin's desktop player, plus the desktop-only output settings. The
 * Android Auto and car sections don't apply.
 */
import { onMounted, ref } from 'vue'
import { getItemProgress, onPlaybackEvent, setOptions, setSkipInterval } from 'tauri-plugin-native-audio-api'
import { player, trackedLists, type OutputDevices } from '@/platform'
import PlaybackSpike from './PlaybackSpike.vue'
import SkipRulesOptions from './SkipRulesOptions.vue'
import SpikePage from './SpikePage.vue'
import { logMsg } from './spike'

// =====================================================================
// SECTION 4e: output (volume, device, media controls)
// =====================================================================
const volume = ref(1)
const outputs = ref<OutputDevices | null>(null)
const mediaControlsOn = ref(true)
const curves = ['quadratic', 'cubic', 'linear'] as const
const curve = ref<(typeof curves)[number]>('quadratic')
// The media keys' previous / next: whole tracks (0) or a seek by this many seconds.
const skipChoices = [0, 10, 30]
const skipSeconds = ref(0)

async function applyVolume() {
  try {
    await player.setVolume(volume.value)
  } catch (e) {
    logMsg(`setVolume FAILED: ${e}`)
  }
}

async function refreshOutputs() {
  try {
    outputs.value = await player.getOutputDevices()
    logMsg(`Outputs: ${outputs.value.devices.length} | selected: ${outputs.value.selected ? deviceName(outputs.value.selected) : 'system default'} | playing on: ${deviceName(outputs.value.active)}`)
  } catch (e) {
    logMsg(`getOutputDevices FAILED: ${e}`)
  }
}

async function chooseOutput(id: string | null) {
  try {
    outputs.value = await player.setOutputDevice(id)
    logMsg(`Output: ${id ? deviceName(id) : 'system default'} -> playing on ${deviceName(outputs.value.active)}`)
  } catch (e) {
    logMsg(`setOutputDevice FAILED: ${e}`)
  }
}

async function toggleMediaControls() {
  mediaControlsOn.value = !mediaControlsOn.value
  await setOptions({ mediaControls: mediaControlsOn.value })
  logMsg(`System media controls: ${mediaControlsOn.value ? 'on' : 'off'}`)
}

async function cycleVolumeCurve() {
  curve.value = curves[(curves.indexOf(curve.value) + 1) % curves.length]
  await setOptions({ volumeCurve: curve.value })
  logMsg(`Volume curve: ${curve.value}`)
}

const deviceName = (id: string | null | undefined) =>
  id ? (outputs.value?.devices.find((d) => d.id === id)?.name ?? id) : '-'

async function cycleSkipInterval() {
  skipSeconds.value = skipChoices[(skipChoices.indexOf(skipSeconds.value) + 1) % skipChoices.length]
  await setSkipInterval(skipSeconds.value)
  logMsg(`Media keys previous / next: ${skipSeconds.value ? `seek ${skipSeconds.value} s` : 'tracks'}`)
}

// =====================================================================
// SECTION 4f: saved data (tracked lists, playback events, progress)
// Like on Android: kept by the plugin and saved between runs.
// =====================================================================
const folderTitles: Record<string, string> = { songs: 'All songs' }
// The plugin has no getter for options, so the test page remembers what it last set.
const progressOn = ref(localStorage.getItem('trackProgress') === 'true')
// Same defaults as the plugin.
const repeatAddedOn = ref(localStorage.getItem('repeatAddedTracks') === 'true')
// Auto-resume: off, paused (load the last queue) or play (load and play it). Older saves were true / false.
const resumeModes = ['paused', 'play', 'off'] as const
type ResumeMode = (typeof resumeModes)[number]
const savedResume = localStorage.getItem('resumeLastQueue')
const resumeMode = ref<ResumeMode>(
  savedResume === 'false' ? 'off' : resumeModes.includes(savedResume as ResumeMode) ? (savedResume as ResumeMode) : 'paused',
)

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

async function setUpSavedData() {
  const lists = trackedLists
  // Section 4's "Start queue" plays the "songs" folder; songs count after 30 s.
  await lists.configure([
    { id: 'recentPlaylists', track: 'folder', limit: 5 },
    { id: 'recentSongs', track: 'item', limit: 10, countAfterSeconds: 30 },
  ])
  // Changes and events from earlier runs arrive first, then live ones.
  await lists.onChange(({ listId, entries }) => {
    const names = entries.map(({ id }) => folderTitles[String(id)] ?? `track ${id}`)
    logMsg(`Tracked list "${listId}": ${names.join(', ') || '(empty)'}`)
  })
  await onPlaybackEvent((event) => {
    logMsg(`Playback event: ${event.type} track ${event.itemId} at ${Math.round(event.positionMs / 1000)} s (${new Date(event.atMs).toLocaleTimeString()})`)
  })
  await setOptions({ trackProgress: progressOn.value, repeatAddedTracks: repeatAddedOn.value, resumeLastQueue: resumeMode.value })
}

async function showLists() {
  for (const id of ['recentPlaylists', 'recentSongs']) {
    const entries = await trackedLists.get(id)
    logMsg(`${id}: ${entries.map((e) => `${folderTitles[String(e.id)] ?? `track ${e.id}`} (${new Date(e.playedAtMs).toLocaleTimeString()})`).join(', ') || '(empty)'}`)
  }
}

async function toggleProgress() {
  progressOn.value = !progressOn.value
  localStorage.setItem('trackProgress', String(progressOn.value))
  await setOptions({ trackProgress: progressOn.value })
  logMsg(`Track progress: ${progressOn.value ? 'on' : 'off'}`)
}

async function showProgress() {
  const entries = await getItemProgress()
  logMsg(`Progress: ${entries.map((e) => `track ${e.itemId} ${Math.round(e.progress * 100)}% (${Math.round(e.positionMs / 1000)} s)`).join(', ') || '(none)'}`)
}

onMounted(async () => {
  try {
    await setUpSavedData()
  } catch (e) {
    logMsg(`Saved data setup FAILED: ${e}`)
  }
  try {
    volume.value = (await player.getState()).volume ?? 1
  } catch {
    // Shown as 100%.
  }
  await refreshOutputs()
})
</script>

<template>
  <SpikePage title="Spike Tests (desktop)">
    <PlaybackSpike />

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4e. Output</h2>
      <label class="flex items-center gap-2 text-sm">
        Volume
        <input v-model.number="volume" type="range" min="0" max="1" step="0.01" class="flex-1" @input="applyVolume" />
        {{ Math.round(volume * 100) }}%
      </label>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="refreshOutputs">Refresh devices</button>
        <button
          class="border rounded px-3 py-1"
          :class="{ 'bg-primary text-primary-foreground': outputs && outputs.selected === null }"
          @click="chooseOutput(null)"
        >
          System default
        </button>
        <button
          v-for="device in outputs?.devices ?? []"
          :key="device.id"
          class="border rounded px-3 py-1"
          :class="{ 'bg-primary text-primary-foreground': outputs?.selected === device.id }"
          @click="chooseOutput(device.id)"
        >
          {{ device.name }}{{ device.isDefault ? ' (default)' : '' }}
        </button>
      </div>
      <p class="text-sm text-muted-foreground">Playing on: {{ deviceName(outputs?.active) }}</p>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="cycleVolumeCurve">Volume curve: {{ curve }}</button>
        <button class="border rounded px-3 py-1" @click="toggleMediaControls">Media controls: {{ mediaControlsOn ? 'on' : 'off' }}</button>
        <button class="border rounded px-3 py-1" @click="cycleSkipInterval">
          Media keys prev/next: {{ skipSeconds ? `seek ${skipSeconds} s` : 'tracks' }}
        </button>
      </div>
    </section>

    <section class="space-y-2 border rounded p-3">
      <h2 class="font-semibold">4f. Saved data</h2>
      <p class="text-sm text-muted-foreground">
        Playback events and tracked list changes show in the log. The queue, volume, output device and options come back after a restart.
      </p>
      <div class="flex flex-wrap gap-2">
        <button class="border rounded px-3 py-1" @click="showLists">Show recently played</button>
        <button class="border rounded px-3 py-1" @click="cycleResume">Auto-resume: {{ resumeMode }}</button>
        <SkipRulesOptions />
        <button class="border rounded px-3 py-1" @click="toggleRepeatAdded">Repeat added tracks: {{ repeatAddedOn ? 'on' : 'off' }}</button>
        <button class="border rounded px-3 py-1" @click="toggleProgress">Track progress: {{ progressOn ? 'on' : 'off' }}</button>
        <button class="border rounded px-3 py-1" @click="showProgress">Show progress</button>
      </div>
    </section>
  </SpikePage>
</template>
