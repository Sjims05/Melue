<script setup lang="ts">
/**
 * Spike tests for Windows / macOS / Linux: the shared sections (SpikePage) and playback (PlaybackSpike),
 * through the native-audio plugin's desktop player, plus the desktop-only output settings. The
 * Android Auto and car sections don't apply.
 */
import { onMounted, ref } from 'vue'
import { setOptions, setSkipInterval } from 'tauri-plugin-native-audio-api'
import { player, type OutputDevices } from '@/platform'
import PlaybackSpike from './PlaybackSpike.vue'
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
    await player!.setVolume(volume.value)
  } catch (e) {
    logMsg(`setVolume FAILED: ${e}`)
  }
}

async function refreshOutputs() {
  try {
    outputs.value = await player!.getOutputDevices()
    logMsg(`Outputs: ${outputs.value.devices.length} | selected: ${outputs.value.selected ? deviceName(outputs.value.selected) : 'system default'} | playing on: ${deviceName(outputs.value.active)}`)
  } catch (e) {
    logMsg(`getOutputDevices FAILED: ${e}`)
  }
}

async function chooseOutput(id: string | null) {
  try {
    outputs.value = await player!.setOutputDevice(id)
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

onMounted(async () => {
  try {
    volume.value = (await player!.getState()).volume ?? 1
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
  </SpikePage>
</template>
