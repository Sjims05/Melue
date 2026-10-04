<script setup lang="ts">
/**
 * Spike options shared by the mobile and desktop pages: what previous and next do
 * (previousRestartsAfterSeconds, nextAtEnd). The plugin saves them; the page remembers what it set.
 */
import { onMounted, ref } from 'vue'
import { setOptions } from 'tauri-plugin-native-audio-api'
import { logMsg } from './spike'

const restartChoices = [3, 0, 10]
const savedRestart = Number(localStorage.getItem('previousRestartsAfterSeconds'))
const restartAfter = ref(restartChoices.includes(savedRestart) && localStorage.getItem('previousRestartsAfterSeconds') !== null ? savedRestart : 3)
const nextAtEnd = ref<'nothing' | 'first'>(localStorage.getItem('nextAtEnd') === 'first' ? 'first' : 'nothing')

async function cycleRestart() {
  restartAfter.value = restartChoices[(restartChoices.indexOf(restartAfter.value) + 1) % restartChoices.length]
  localStorage.setItem('previousRestartsAfterSeconds', String(restartAfter.value))
  await setOptions({ previousRestartsAfterSeconds: restartAfter.value })
  logMsg(`Previous: ${restartAfter.value ? `restarts the song after ${restartAfter.value} s` : 'always the previous song'}`)
}

async function toggleNextAtEnd() {
  nextAtEnd.value = nextAtEnd.value === 'first' ? 'nothing' : 'first'
  localStorage.setItem('nextAtEnd', nextAtEnd.value)
  await setOptions({ nextAtEnd: nextAtEnd.value })
  logMsg(`Next on the last song: ${nextAtEnd.value === 'first' ? 'first song, paused' : 'nothing'}`)
}

onMounted(async () => {
  try {
    await setOptions({ previousRestartsAfterSeconds: restartAfter.value, nextAtEnd: nextAtEnd.value })
  } catch (e) {
    logMsg(`Previous / next options FAILED: ${e}`)
  }
})
</script>

<template>
  <button class="border rounded px-3 py-1" @click="cycleRestart">
    Previous restarts after: {{ restartAfter ? `${restartAfter} s` : 'never' }}
  </button>
  <button class="border rounded px-3 py-1" @click="toggleNextAtEnd">
    Next at end: {{ nextAtEnd === 'first' ? 'first song' : 'nothing' }}
  </button>
</template>
