<script setup lang="ts">
/**
 * Spike tests for Android / iOS: the shared sections (SpikePage, PlaybackSpike) plus the plugin's
 * mobile-only options, which use the plugin directly. The Android Auto library and buttons are the real
 * app's (auto.rs, src/library/playback.ts).
 */
import { onMounted, ref } from 'vue'
import { setOptions } from 'tauri-plugin-native-audio-api'
import PlaybackSpike from './PlaybackSpike.vue'
import SkipRulesOptions from './SkipRulesOptions.vue'
import SpikePage from './SpikePage.vue'
import { logMsg } from './spike'

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

onMounted(async () => {
  try {
    await setOptions({
      resumeLastQueue: resumeMode.value,
      repeatAddedTracks: repeatAddedOn.value,
      keepAliveWhileCarConnected: carKeepAliveOn.value,
      pausedKeepAliveMinutes: keepAliveMinutes.value,
    })
  } catch (e) {
    logMsg(`Setup FAILED: ${e}`)
  }
})
</script>

<template>
  <SpikePage title="Spike Tests">
    <PlaybackSpike />

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
